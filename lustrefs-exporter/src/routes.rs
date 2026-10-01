// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    Error,
    client::ClientLabels,
    histogram::HistogramEncoding,
    jobstats::{JobstatMetrics, jobstats_stream},
    metrics::{self, Metrics, fold_records},
    stream::lctl_records,
};
use axum::{
    BoxError, Router,
    body::Body,
    error_handling::HandleErrorLayer,
    extract::{Query, State},
    http::{StatusCode, header::CONTENT_TYPE},
    response::{IntoResponse, Response},
    routing::get,
};
use lustre_collector::{
    parse_lnetctl_global_show, parse_lnetctl_output, parse_lnetctl_stats, parser,
};
use prometheus_client::{encoding::text::encode, registry::Registry};
use serde::Deserialize;
use std::{
    borrow::Cow,
    collections::HashSet,
    io::{self, BufReader},
    os::unix::process::ExitStatusExt,
};
use tokio::{
    io::AsyncReadExt as _,
    process::{Child, ChildStdout, Command},
};
use tower::{
    ServiceBuilder, limit::GlobalConcurrencyLimitLayer, load_shed::LoadShedLayer,
    timeout::TimeoutLayer,
};
use tower_http::compression::CompressionLayer;

#[derive(Debug, Deserialize)]
pub struct Params {
    // Only enable jobstats if "jobstats=true"
    #[serde(default)]
    jobstats: bool,
    // Reset mdt md_stats between scrapes if "reset_mdt_md_stats=true"
    #[serde(default)]
    reset_mdt_md_stats: bool,
}

const TIMEOUT_DURATION_SECS: u64 = 120;

#[derive(Clone, Copy, Debug, Default)]
pub struct ExporterConfig {
    pub client_labels: ClientLabels,
    pub histogram_encoding: HistogramEncoding,
}

pub fn app(config: ExporterConfig) -> Router {
    let load_shedder = ServiceBuilder::new()
        .layer(HandleErrorLayer::new(handle_error))
        .layer(LoadShedLayer::new())
        .layer(TimeoutLayer::new(std::time::Duration::from_secs(
            TIMEOUT_DURATION_SECS,
        )))
        .layer(GlobalConcurrencyLimitLayer::new(10))
        .layer(CompressionLayer::new());

    Router::new()
        .route("/metrics", get(scrape))
        .layer(load_shedder)
        .with_state(config)
}

pub async fn handle_error(error: BoxError) -> impl IntoResponse {
    if error.is::<tower::timeout::error::Elapsed>() {
        return (StatusCode::REQUEST_TIMEOUT, Cow::from("request timed out"));
    }

    if error.is::<tower::load_shed::error::Overloaded>() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Cow::from("service is overloaded, try again later"),
        );
    }

    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Cow::from(format!("Unhandled internal error: {error}")),
    )
}

pub fn jobstats_metrics_cmd() -> Command {
    let mut cmd = Command::new("lctl");

    cmd.arg("get_param")
        .args(["obdfilter.*OST*.job_stats", "mdt.*.job_stats"])
        .env("LC_ALL", "C")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);

    cmd
}

pub fn lustre_metrics_output() -> Command {
    let mut cmd = Command::new("lctl");

    // The stderr classification matches strerror text.
    cmd.arg("get_param")
        .args(parser::params())
        .env("LC_ALL", "C")
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);

    cmd
}

struct Piped {
    child: Child,
    name: &'static str,
    stderr: tokio::task::JoinHandle<io::Result<String>>,
}

/// stderr is drained in the background so a chatty `lctl` cannot stall on a
/// full pipe.
fn spawn_piped(
    mut cmd: Command,
    name: &'static str,
) -> Result<(Piped, BufReader<std::process::ChildStdout>), Error> {
    let mut child = cmd.spawn()?;

    let stdout = child.stdout.take().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("stdout missing for {name}"),
        )
    })?;

    let mut stderr = child.stderr.take().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("stderr missing for {name}"),
        )
    })?;
    let stderr = tokio::spawn(async move {
        let mut bytes = Vec::new();

        stderr.read_to_end(&mut bytes).await?;

        Ok(String::from_utf8_lossy(&bytes).into_owned())
    });

    Ok((
        Piped {
            child,
            name,
            stderr,
        },
        BufReader::with_capacity(128 * 1_024, blocking(stdout)?),
    ))
}

/// tokio switches the fd back to blocking mode on the way out.
fn blocking(stdout: ChildStdout) -> Result<std::process::ChildStdout, Error> {
    Ok(std::process::ChildStdout::from(stdout.into_owned_fd()?))
}

/// A pattern that matches nothing on the node, which the exporter's list
/// always has; a parameter that could not be read prints `read_param:`.
fn is_expected_stderr(line: &str) -> bool {
    line.contains("param_path '") && line.contains("No such file or directory")
}

/// The command was still writing when this side stopped reading.
const SIGPIPE: i32 = 13;

impl Piped {
    /// A signal death means truncated output, so the scrape fails; unexpected
    /// stderr is counted. The exit status is 2 whenever any pattern matched
    /// nothing, so it is only logged.
    async fn finish(mut self, metrics: &Metrics) -> Result<(), Error> {
        let status = self.child.wait().await?;
        let stderr = self.stderr.await.map_err(io::Error::other)??;

        if let Some(signal) = status.signal()
            && signal != SIGPIPE
        {
            return Err(io::Error::other(format!(
                "{} was killed by signal {signal}; its output is incomplete",
                self.name
            ))
            .into());
        }

        let unexpected: Vec<&str> = stderr
            .lines()
            .filter(|l| !l.trim().is_empty() && !is_expected_stderr(l))
            .collect();

        if let Some(first) = unexpected.first() {
            metrics.record_command_errors(self.name, unexpected.len() as u64, first.trim());
        } else {
            tracing::debug!(
                "{} exited with {status}; stderr: {}",
                self.name,
                stderr.trim_end()
            );
        }

        Ok(())
    }
}

async fn reset_mdt_md_stats() -> Result<(), Error> {
    let output = Command::new("lctl")
        .args(["set_param", "mdt.*.md_stats", "0"])
        .kill_on_drop(true)
        .output()
        .await
        .map_err(|e| Error::MdtStatsReset(format!("Failed to execute reset command: {e}"), None))?;

    if !output.status.success() {
        return Err(Error::MdtStatsReset(
            String::from_utf8_lossy(&output.stderr).to_string(),
            output.status.code(),
        ));
    }

    Ok(())
}

pub fn net_show_output() -> Command {
    let mut cmd = Command::new("lnetctl");

    cmd.args(["net", "show", "-v", "4"]).kill_on_drop(true);

    cmd
}

pub fn lnet_stats_output() -> Command {
    let mut cmd = Command::new("lnetctl");

    cmd.args(["stats", "show"]).kill_on_drop(true);

    cmd
}

pub fn lnet_global_output() -> Command {
    let mut cmd = Command::new("lnetctl");

    cmd.args(["global", "show"]).kill_on_drop(true);

    cmd
}

/// Main metrics scraping endpoint handler for the Prometheus exporter.
///
/// This function serves as the primary HTTP handler for the `/metrics` endpoint,
/// collecting and formatting Lustre filesystem metrics in Prometheus format.
/// It orchestrates the collection of both standard Lustre statistics and optional
/// jobstats data based on query parameters.
///
/// # Arguments
///
/// * `Query(params)` - Query parameters extracted from the HTTP request
/// * `State(state)` - Shared application state containing the command handler
///
/// # Query Parameters
///
/// * `jobstats` - Optional boolean parameter to enable jobstats collection
///   (e.g., `/metrics?jobstats=true`)
///
/// # Returns
///
/// * `Ok(Response<Body>)` - HTTP response with Prometheus-formatted metrics
/// * `Err(Error)` - Error if metric collection or formatting fails
///
/// # Processing Flow
///
/// 1. **Initialize**: Creates a new Prometheus registry and default metrics structures
/// 2. **Conditional Jobstats**: If `jobstats=true`, collects and registers jobstats metrics
/// 3. **Standard Metrics**: Always collects standard Lustre and LNet statistics
/// 4. **Registration**: Registers all populated metrics with the registry
/// 5. **Encoding**: Encodes metrics in Prometheus text format
/// 6. **Response**: Returns HTTP 200 response with metrics as body
///
/// # Performance Considerations
///
/// - Jobstats collection can be resource-intensive and is optional but will
///   be run within a spawned task.
/// - Standard metrics collection runs commands concurrently for efficiency
/// - `lctl get_param` output is parsed one parameter at a time on a blocking
///   thread, so neither the raw text nor its records are ever held in full
/// - Only metrics with actual data are registered to keep output clean
pub async fn scrape(
    State(config): State<ExporterConfig>,
    Query(params): Query<Params>,
) -> Result<Response<Body>, Error> {
    let mut registry = Registry::default();

    // Build the lustre stats
    let mut opentelemetry_metrics = Metrics::new(config.client_labels, config.histogram_encoding);
    let mut set = HashSet::new();

    if params.jobstats {
        match spawn_piped(jobstats_metrics_cmd(), "lctl get_param job_stats") {
            Ok((child, reader)) => {
                let (metrics, truncated) =
                    jobstats_stream(reader, JobstatMetrics::default()).await?;

                metrics.register_metric(&mut registry);

                if truncated {
                    opentelemetry_metrics.record_parse_errors(
                        "jobstats",
                        1,
                        "the jobstats parser stopped at an unexpected line",
                    );
                }

                child.finish(&opentelemetry_metrics).await?;
            }
            Err(e) => {
                tracing::debug!("Error while spawning lctl jobstats: {e}");
            }
        }
    }

    let (child, reader) = spawn_piped(lustre_metrics_output(), "lctl get_param")?;

    (opentelemetry_metrics, set) = tokio::task::spawn_blocking(move || {
        fold_records(
            lctl_records(reader),
            "lctl",
            &mut opentelemetry_metrics,
            &mut set,
        )?;

        Ok::<_, Error>((opentelemetry_metrics, set))
    })
    .await??;

    child.finish(&opentelemetry_metrics).await?;

    // Reset md_stats if requested (after collection)
    if params.reset_mdt_md_stats {
        reset_mdt_md_stats().await?;
    }

    let mut output = vec![];

    let lnetctl = net_show_output().output().await?;

    let mut lnetctl_output = parse_lnetctl_output(&lnetctl.stdout)?;

    output.append(&mut lnetctl_output);

    let lnetctl_stats_output = lnet_stats_output().output().await?;

    let mut lnetctl_stats_record = parse_lnetctl_stats(&lnetctl_stats_output.stdout)?;

    output.append(&mut lnetctl_stats_record);

    let lnetctl_global_output = lnet_global_output().output().await?;

    let mut lnetctl_global_record = parse_lnetctl_global_show(&lnetctl_global_output.stdout)?;

    output.append(&mut lnetctl_global_record);

    for record in &output {
        metrics::process_record(record, &mut opentelemetry_metrics, &mut set);
    }

    opentelemetry_metrics.register_metric(&mut registry);

    let mut buffer = String::new();
    encode(&mut buffer, &registry)?;

    let resp = Response::builder()
        .status(StatusCode::OK)
        .header(
            CONTENT_TYPE,
            "application/openmetrics-text; version=1.0.0; charset=utf-8",
        )
        .body(Body::from(buffer))?;

    Ok(resp)
}

#[cfg(test)]
mod tests {
    use crate::routes::{
        jobstats_metrics_cmd, lnet_global_output, lnet_stats_output, lustre_metrics_output,
        net_show_output,
    };
    use axum::{
        Router,
        body::{Body, to_bytes},
        extract::Request,
        http::StatusCode,
    };
    use commandeer_test::commandeer;
    use serial_test::serial;
    use tokio::{process::Command, task::JoinSet};
    use tower::ServiceExt as _;

    /// Create a new Axum app with the provided state and a Request
    /// to scrape the metrics endpoint.
    fn get_app() -> (Request<Body>, Router) {
        let app = crate::routes::app(crate::routes::ExporterConfig::default());

        let request = Request::builder()
            .uri("/metrics?jobstats=true")
            .method("GET")
            .body(Body::empty())
            .unwrap();

        (request, app)
    }

    #[commandeer(Replay, "lctl", "lnetctl")]
    #[tokio::test]
    #[serial]
    async fn test_metrics_endpoint_is_idempotent() -> Result<(), Box<dyn std::error::Error>> {
        let (request, app) = get_app();

        let resp = app.oneshot(request).await.unwrap();

        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let original_body_str = std::str::from_utf8(&body).unwrap();

        let (request, app) = get_app();

        let resp = app.oneshot(request).await.unwrap();

        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let body_str = std::str::from_utf8(&body).unwrap();

        assert_eq!(original_body_str, body_str);

        insta::assert_snapshot!(original_body_str);

        Ok(())
    }

    #[commandeer(Replay, "lctl", "lnetctl")]
    #[tokio::test]
    #[serial]
    async fn test_unmatched_params_are_not_errors() {
        let app = crate::routes::app(crate::routes::ExporterConfig::default());

        let request = Request::builder()
            .uri("/metrics")
            .method("GET")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(request).await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let body = std::str::from_utf8(&body).unwrap();

        assert!(!body.contains("lustre_exporter_"), "{body}");
        assert!(body.contains("lustre_mem_used "));
    }

    #[commandeer(Replay, "lctl", "lnetctl")]
    #[tokio::test]
    #[serial]
    async fn test_bad_block_and_stderr_error_are_counted() {
        let app = crate::routes::app(crate::routes::ExporterConfig::default());

        let request = Request::builder()
            .uri("/metrics")
            .method("GET")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(request).await.unwrap();

        assert_eq!(resp.status(), StatusCode::OK);

        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let body = std::str::from_utf8(&body).unwrap();

        assert!(body.contains("lustre_exporter_parse_errors{source=\"lctl\"} 1\n"));
        assert!(body.contains("lustre_exporter_command_errors{command=\"lctl get_param\"} 1\n"));
        assert!(body.contains("lustre_mem_used "));
    }

    async fn finish(script: &str) -> (Result<(), crate::Error>, String) {
        let mut cmd = Command::new("sh");
        cmd.args(["-c", script])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        let (child, mut reader) = super::spawn_piped(cmd, "sh").unwrap();
        let mut out = String::new();
        std::io::Read::read_to_string(&mut reader, &mut out).unwrap();
        let metrics = crate::metrics::Metrics::default();
        let result = child.finish(&metrics).await;
        let mut registry = prometheus_client::registry::Registry::default();
        metrics.register_metric(&mut registry);
        let mut text = String::new();
        prometheus_client::encoding::text::encode(&mut text, &registry).unwrap();
        (result, text)
    }

    #[tokio::test]
    async fn finish_classifies_exit_and_stderr() {
        let (killed, _) = finish("kill -KILL $$").await;
        assert!(killed.is_err());

        let (pipe, text) = finish("kill -PIPE $$").await;
        assert!(pipe.is_ok());
        assert!(!text.contains("lustre_exporter_command_errors"));

        let (exit3, text) =
            finish("echo x; echo 'error: get_param: something bad' >&2; exit 3").await;
        assert!(exit3.is_ok());
        assert!(
            text.contains("lustre_exporter_command_errors{command=\"sh\"} 1\n"),
            "{text}"
        );

        let (enoent, text) = finish(
            "echo \"error: get_param: param_path 'llite/*/stats': No such file or directory\" >&2; exit 2",
        )
        .await;
        assert!(enoent.is_ok());
        assert!(!text.contains("lustre_exporter_command_errors"), "{text}");
    }

    #[commandeer(Replay, "lctl", "lnetctl")]
    #[tokio::test]
    #[serial]
    async fn test_app_function() {
        let (request, app) = get_app();

        let response = app.oneshot(request).await.unwrap();

        assert!(response.status().is_success())
    }

    #[commandeer(Replay, "lctl", "lnetctl")]
    #[tokio::test]
    #[serial]
    async fn test_app_routes() {
        let app = crate::routes::app(crate::routes::ExporterConfig::default());

        // Test that the /metrics route exists
        let request = Request::builder()
            .uri("/metrics")
            .method("GET")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert!(response.status().is_success())
    }

    #[commandeer(Replay, "lctl", "lnetctl")]
    #[tokio::test]
    #[serial]
    async fn test_concurrent_requests() {
        let app = crate::routes::app(crate::routes::ExporterConfig::default());

        // Test that concurrency limiting works by sending multiple requests
        // This test verifies the load_shed layer is applied
        let mut handles = JoinSet::new();

        // Send 15 requests (more than the 10 limit)
        for _ in 0..15 {
            let app = app.clone();

            handles.spawn(async move {
                let request = Request::builder()
                    .uri("/metrics")
                    .method("GET")
                    .body(Body::empty())
                    .unwrap();

                app.oneshot(request).await
            });
        }

        // Wait for all requests to complete
        let result = handles
            .join_all()
            .await
            .into_iter()
            .collect::<Result<Vec<_>, _>>();

        // Some requests should succeed or fail based on system state,
        // but none should panic
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_handle_error() {
        use crate::routes::handle_error;
        use axum::{BoxError, http::StatusCode, response::IntoResponse};

        // Test timeout error
        let timeout_error = Box::new(tower::timeout::error::Elapsed::new()) as BoxError;
        let response = handle_error(timeout_error).await.into_response();

        assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body_str = std::str::from_utf8(&body).unwrap();

        assert_eq!(body_str, "request timed out");

        // Test overloaded error
        let overloaded_error = Box::new(tower::load_shed::error::Overloaded::new()) as BoxError;
        let response = handle_error(overloaded_error).await.into_response();

        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body_str = std::str::from_utf8(&body).unwrap();

        assert_eq!(body_str, "service is overloaded, try again later");

        // Test generic/unhandled error
        let generic_error = Box::new(std::io::Error::other("some random error")) as BoxError;

        let response = handle_error(generic_error).await.into_response();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body_str = std::str::from_utf8(&body).unwrap();

        assert!(body_str.starts_with("Unhandled internal error:"));
    }

    #[commandeer(Replay, "lctl")]
    #[tokio::test]
    #[serial]
    async fn test_jobstats_metrics_cmd_with_mock() {
        let output = jobstats_metrics_cmd().output().await.unwrap();

        insta::assert_snapshot!(String::from_utf8(output.stdout).unwrap());
    }

    #[commandeer(Replay, "lctl")]
    #[tokio::test]
    #[serial]
    async fn test_lustre_metrics_output_with_mock() {
        let output = lustre_metrics_output().output().await.unwrap();

        insta::assert_snapshot!(String::from_utf8(output.stdout).unwrap());
    }

    #[commandeer(Replay, "lnetctl")]
    #[tokio::test]
    #[serial]
    async fn test_net_show_output_with_mock() {
        let output = net_show_output().output().await.unwrap();

        insta::assert_snapshot!(String::from_utf8(output.stdout).unwrap());
    }

    #[commandeer(Replay, "lnetctl")]
    #[tokio::test]
    #[serial]
    async fn test_lnet_stats_output_with_mock() {
        let output = lnet_stats_output().output().await.unwrap();

        insta::assert_snapshot!(String::from_utf8(output.stdout).unwrap());
    }

    /// Test that demonstrates both lustre_health_sensitivity (global) and
    /// lustre_health_value (per-NID) metrics from LNet health monitoring.
    ///
    /// This test uses mock data that includes:
    /// - lnetctl global show: provides health_sensitivity (global gauge)
    /// - lnetctl net show -v 4: provides health_value for each NID (per-NID gauge)
    #[commandeer(Replay, "lnetctl")]
    #[tokio::test]
    #[serial]
    async fn test_lnet_health_metrics() -> Result<(), Box<dyn std::error::Error>> {
        use crate::metrics::{self, Metrics};
        use lustre_collector::{parse_lnetctl_global_show, parse_lnetctl_output};
        use prometheus_client::{encoding::text::encode, registry::Registry};

        // Collect LNet network interface stats (health_value per NID)
        let net_show = net_show_output().output().await?;
        let mut lnet_records = parse_lnetctl_output(&net_show.stdout)?;

        // Collect LNet global stats (health_sensitivity)
        let global_show = lnet_global_output().output().await?;
        let mut global_records = parse_lnetctl_global_show(&global_show.stdout)?;

        // Combine all records
        let mut all_records = Vec::new();
        all_records.append(&mut lnet_records);
        all_records.append(&mut global_records);

        // Build metrics
        let mut registry = Registry::default();
        let mut metrics = Metrics::default();
        metrics::build_lustre_stats(&all_records, &mut metrics);
        metrics.register_metric(&mut registry);

        // Encode to Prometheus text format
        let mut output = String::new();
        encode(&mut output, &registry)?;

        // Verify the output contains both health metrics
        assert!(output.contains("lustre_health_sensitivity"));
        assert!(output.contains("lustre_health_value"));

        // Create snapshot for the complete output
        insta::assert_snapshot!(output);

        Ok(())
    }

    #[commandeer(Replay, "lctl", "lnetctl")]
    #[tokio::test]
    #[serial]
    async fn test_jobstats_with_stderr_output() -> Result<(), Box<dyn std::error::Error>> {
        let (request, app) = get_app();

        let resp = app.oneshot(request).await.unwrap();

        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let original_body_str = std::str::from_utf8(&body).unwrap();

        insta::assert_snapshot!(original_body_str);

        Ok(())
    }

    /// Covers reset_mdt_md_stats() success path: command execution and Ok(()) return.
    #[commandeer(Replay, "lctl")]
    #[tokio::test]
    #[serial]
    async fn test_reset_mdt_md_stats_returns_ok_on_successful_lctl_command() {
        use crate::routes::reset_mdt_md_stats;

        let result = reset_mdt_md_stats().await;

        assert!(result.is_ok());
    }

    /// Covers reset_mdt_md_stats() error path: captures stderr, exit code, and returns MdStatsReset error.
    #[commandeer(Replay, "lctl")]
    #[tokio::test]
    #[serial]
    async fn test_reset_mdt_md_stats_returns_error_with_stderr_and_exit_code_on_failure() {
        use crate::routes::reset_mdt_md_stats;

        let result = reset_mdt_md_stats().await;

        match result {
            Err(crate::Error::MdtStatsReset(msg, exit_code)) => {
                assert!(!msg.is_empty());
                assert_eq!(exit_code, Some(1));
                assert!(msg.contains("Permission denied") || msg.contains("error"));
            }
            Err(e) => panic!("Expected MdStatsReset error, got: {e:?}"),
            Ok(_) => panic!("Expected error, got success"),
        }
    }
}

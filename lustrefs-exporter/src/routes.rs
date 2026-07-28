// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    Error,
    jobstats::{JobstatMetrics, jobstats_stream},
    metrics::{self, Metrics},
};
use axum::{
    BoxError, Router,
    body::Body,
    error_handling::HandleErrorLayer,
    extract::Query,
    http::{StatusCode, header::CONTENT_TYPE},
    response::{IntoResponse, Response},
    routing::get,
};
use lustre_collector::{
    parse_lctl_output, parse_lnetctl_global_show, parse_lnetctl_output, parse_lnetctl_stats, parser,
};
use prometheus_client::{encoding::text::encode, registry::Registry};
use serde::Deserialize;
use std::{
    borrow::Cow,
    io::{self, BufRead as _, BufReader},
};
use tokio::process::Command;
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

pub fn app() -> Router {
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

pub fn jobstats_metrics_cmd() -> std::process::Command {
    let mut cmd = std::process::Command::new("lctl");

    cmd.arg("get_param")
        .args(["obdfilter.*OST*.job_stats", "mdt.*.job_stats"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());

    cmd
}

pub fn lustre_metrics_output() -> Command {
    let mut cmd = Command::new("lctl");

    cmd.arg("get_param")
        .args(parser::params())
        .kill_on_drop(true);

    cmd
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
/// - Only metrics with actual data are registered to keep output clean
pub async fn scrape(Query(params): Query<Params>) -> Result<Response<Body>, Error> {
    let mut registry = Registry::default();

    // Build the lustre stats
    let mut opentelemetry_metrics = Metrics::default();

    if params.jobstats {
        let child = tokio::task::spawn_blocking(move || {
            let child = jobstats_metrics_cmd().spawn()?;

            Ok::<_, Error>(child)
        })
        .await?;

        match child {
            Ok(mut child) => {
                let reader = BufReader::with_capacity(
                    128 * 1_024,
                    child.stdout.take().ok_or(io::Error::new(
                        io::ErrorKind::NotFound,
                        "stdout missing for lctl jobstats call.",
                    ))?,
                );

                let reader_stderr = BufReader::new(child.stderr.take().ok_or(io::Error::new(
                    io::ErrorKind::NotFound,
                    "stderr missing for lctl jobstats call.",
                ))?);

                tokio::task::spawn(async move {
                    for line in reader_stderr.lines().map_while(Result::ok) {
                        tracing::debug!("stderr: {line}");
                    }
                });

                tokio::task::spawn_blocking(move || {
                    if let Err(e) = child.wait() {
                        tracing::debug!("Unexpected error when waiting for child: {e}");
                    }
                });

                let handle = jobstats_stream(reader, JobstatMetrics::default());

                let metrics = handle.await?;

                metrics.register_metric(&mut registry);
            }
            Err(e) => {
                tracing::debug!("Error while spawning lctl jobstats: {e}");
            }
        }
    }

    let mut output = vec![];

    let lctl = lustre_metrics_output().output().await?;

    let mut lctl_output = parse_lctl_output(&lctl.stdout)?;

    output.append(&mut lctl_output);

    // Reset md_stats if requested (after collection)
    if params.reset_mdt_md_stats {
        reset_mdt_md_stats().await?;
    }

    let lnetctl = net_show_output().output().await?;

    let mut lnetctl_output = parse_lnetctl_output(&lnetctl.stdout)?;

    output.append(&mut lnetctl_output);

    let lnetctl_stats_output = lnet_stats_output().output().await?;

    let mut lnetctl_stats_record = parse_lnetctl_stats(&lnetctl_stats_output.stdout)?;

    output.append(&mut lnetctl_stats_record);

    let lnetctl_global_output = lnet_global_output().output().await?;

    let mut lnetctl_global_record = parse_lnetctl_global_show(&lnetctl_global_output.stdout)?;

    output.append(&mut lnetctl_global_record);

    // Build and register Lustre metrics
    metrics::build_lustre_stats(&output, &mut opentelemetry_metrics);
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
    };
    use commandeer_test::commandeer;
    use serial_test::serial;
    use std::io::{self, BufReader, Read};
    use tokio::task::JoinSet;
    use tower::ServiceExt as _;

    /// Create a new Axum app with the provided state and a Request
    /// to scrape the metrics endpoint.
    fn get_app() -> (Request<Body>, Router) {
        let app = crate::routes::app();

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
    async fn test_app_function() {
        let (request, app) = get_app();

        let response = app.oneshot(request).await.unwrap();

        assert!(response.status().is_success())
    }

    #[commandeer(Replay, "lctl", "lnetctl")]
    #[tokio::test]
    #[serial]
    async fn test_app_routes() {
        let app = crate::routes::app();

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
        let app = crate::routes::app();

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
    #[test]
    #[serial]
    fn test_jobstats_metrics_cmd_with_mock() {
        let mut child = jobstats_metrics_cmd()
            .spawn()
            .expect("Failed to spawn child.");

        let mut reader = BufReader::with_capacity(
            128 * 1_024,
            child
                .stdout
                .take()
                .ok_or(io::Error::new(
                    io::ErrorKind::NotFound,
                    "stdout missing for lctl jobstats call.",
                ))
                .unwrap(),
        );

        let mut buff = String::new();
        reader.read_to_string(&mut buff).unwrap();

        child.wait().expect("Failed to wait for child process");

        insta::assert_snapshot!(buff);
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

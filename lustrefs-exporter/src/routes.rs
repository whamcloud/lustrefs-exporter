// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    Error, init_opentelemetry,
    jobstats::opentelemetry::OpenTelemetryMetricsJobstats,
    openmetrics::{self, OpenTelemetryMetrics},
};
use axum::{
    BoxError, Router,
    body::Body,
    error_handling::HandleErrorLayer,
    extract::Query,
    http::{self, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use lustre_collector::{parse_lctl_output, parse_lnetctl_output, parse_lnetctl_stats, parser};
use opentelemetry::metrics::MeterProvider;
use prometheus::{Encoder as _, TextEncoder};
use serde::Deserialize;
use std::{
    borrow::Cow,
    convert::Infallible,
    io::{self, BufRead, BufReader},
    sync::Arc,
};
use tokio::process::Command;
use tokio_stream::StreamExt as _;
use tower::ServiceBuilder;

#[derive(Debug, Deserialize)]
pub struct Params {
    // Only enable jobstats if "jobstats=true"
    #[serde(default)]
    jobstats: bool,
}

pub fn app() -> Router {
    let load_shedder = ServiceBuilder::new()
        .layer(HandleErrorLayer::new(handle_error))
        .load_shed()
        .concurrency_limit(10); // Max 10 concurrent scrape

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

pub fn jobstats_metrics_cmd() -> Result<std::process::Child, std::io::Error> {
    let child = std::process::Command::new("lctl")
        .arg("get_param")
        .args(["obdfilter.*OST*.job_stats", "mdt.*.job_stats"])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    Ok(child)
}

pub async fn lustre_metrics_output() -> Result<std::process::Output, std::io::Error> {
    let output = Command::new("lctl")
        .arg("get_param")
        .args(parser::params())
        .kill_on_drop(true)
        .output()
        .await?;

    Ok(output)
}

pub async fn net_show_output() -> Result<std::process::Output, std::io::Error> {
    let lnetctl = Command::new("lnetctl")
        .args(["net", "show", "-v", "4"])
        .kill_on_drop(true)
        .output()
        .await?;

    Ok(lnetctl)
}

pub async fn lnet_stats_output() -> Result<std::process::Output, std::io::Error> {
    let lnetctl = Command::new("lnetctl")
        .args(["stats", "show"])
        .kill_on_drop(true)
        .output()
        .await?;

    Ok(lnetctl)
}

pub async fn scrape(Query(params): Query<Params>) -> Result<Response<Body>, Error> {
    let (provider, registry) = init_opentelemetry()?;

    let meter = provider.meter("lustre");
    let jobstats = if params.jobstats {
        let child = tokio::task::spawn_blocking(move || {
            let child = jobstats_metrics_cmd()?;

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
                        tracing::debug!("stderr: {}", line);
                    }
                });

                let otel_jobstats = Arc::new(OpenTelemetryMetricsJobstats::new(&meter));

                tokio::task::spawn_blocking(move || {
                    if let Err(e) = child.wait() {
                        tracing::debug!("Unexpected error when waiting for child: {e}");
                    }
                });

                let handle =
                    crate::jobstats::opentelemetry::jobstats_stream(reader, otel_jobstats.clone());

                handle.await?;

                // Encode metrics to string
                let encoder = TextEncoder::new();
                let metric_families = registry.gather();
                let mut output = Vec::new();
                encoder.encode(&metric_families, &mut output)?;

                let output = String::from_utf8_lossy(&output).to_string();

                Some(output)
            }
            Err(e) => {
                tracing::debug!("Error while spawning lctl jobstats: {e}");

                None
            }
        }
    } else {
        None
    };

    let mut output = vec![];

    let lctl = lustre_metrics_output().await?;

    let mut lctl_output = parse_lctl_output(&lctl.stdout)?;

    output.append(&mut lctl_output);

    let lnetctl = net_show_output().await?;

    let lnetctl_stats = std::str::from_utf8(&lnetctl.stdout)?;

    let mut lnetctl_output = parse_lnetctl_output(lnetctl_stats)?;

    output.append(&mut lnetctl_output);

    let lnetctl_stats_output = lnet_stats_output().await?;

    let mut lnetctl_stats_record =
        parse_lnetctl_stats(std::str::from_utf8(&lnetctl_stats_output.stdout)?)?;

    output.append(&mut lnetctl_stats_record);

    let opentelemetry_metrics = OpenTelemetryMetrics::new(meter.clone());

    let mut lustre_stats = vec![];
    let encoder = TextEncoder::new();
    let metric_families = registry.gather();
    let _encoder_results = encoder.encode(&metric_families, &mut lustre_stats);

    // Build OTEL metrics
    openmetrics::build_lustre_stats(&output, opentelemetry_metrics);

    let mut buffer = vec![];
    let encoder = TextEncoder::new();
    let metric_families = registry.gather();
    encoder.encode(&metric_families, &mut buffer).unwrap();

    let lustre_stats = String::from_utf8_lossy(&buffer).to_string();

    let body = if let Some(stream) = jobstats {
        let merged = tokio_stream::once(Ok::<_, Infallible>(lustre_stats))
            .chain(tokio_stream::once(Ok(stream)));

        Body::from_stream(merged)
    } else {
        tracing::debug!("Jobstats collection disabled");

        Body::from(lustre_stats)
    };

    let mut response_builder = Response::builder().status(StatusCode::OK);

    let headers = response_builder.headers_mut();
    if let Ok(content_type) = encoder.format_type().parse::<HeaderValue>() {
        if let Some(headers) = headers {
            headers.insert(http::header::CONTENT_TYPE, content_type);
        }
    }
    let resp = response_builder.body(body)?;

    Ok(resp)
}

#[cfg(test)]
#[cfg(debug_assertions)]
mod tests {
    use crate::routes::{
        jobstats_metrics_cmd, lnet_stats_output, lustre_metrics_output, net_show_output,
    };
    use axum::{
        Router,
        body::{Body, to_bytes},
        extract::Request,
    };
    use injectorpp::interface::injector::*;
    use std::{
        env,
        io::{self, BufReader, Read},
        os::unix::process::ExitStatusExt as _,
        path::PathBuf,
        process::{ExitStatus, Output},
    };
    use tokio::task::JoinSet;
    use tower::ServiceExt as _;

    // Prepare the test environment. This includes:
    // 1. Putting the mock lctl binary in the PATH environment variable
    // 2. Putting the mock lnetctl binary in the PATH environment variable
    pub fn setup_env() {
        let mock_bin = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("mock_bins");

        let current_path = env::var("PATH").unwrap_or_default();

        let new_path = format!("{current_path}:{}", mock_bin.display());

        unsafe {
            env::set_var("PATH", new_path);
        }
    }

    fn mock_command_calls(injector: &mut InjectorPP) {
        injector
            .when_called(injectorpp::func!(
                fn (jobstats_metrics_cmd)() -> Result<std::process::Child, std::io::Error>
            ))
            .will_execute(injectorpp::fake!(
                func_type: fn() -> Result<std::process::Child, std::io::Error>,
                returns: std::process::Command::new("cat")
                    .arg("fixtures/jobstats_only/2.14.0_162.txt")
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped())
                    .spawn()
            ));

        injector
            .when_called_async(injectorpp::async_func!(
                lustre_metrics_output(), Result<std::process::Output, std::io::Error>
            ))
            .will_return_async(injectorpp::async_return!(
                Ok(Output {
                    status: ExitStatus::from_raw(0),
                    stdout: include_str!("../../lustre-collector/src/fixtures/valid/lustre-2.14.0_ddn133/2.14.0_ddn133_quota.txt").as_bytes().to_vec(),
                    stderr: b"".to_vec(),
                }),
                Result<std::process::Output, std::io::Error>
            ));

        injector
            .when_called_async(injectorpp::async_func!(
                net_show_output(), Result<std::process::Output, std::io::Error>
            ))
            .will_return_async(injectorpp::async_return!(
                Ok(Output {
                    status: ExitStatus::from_raw(0),
                    stdout: include_str!("../fixtures/lnetctl_net_show.txt").as_bytes().to_vec(),
                    stderr: b"".to_vec(),
                }),
                Result<std::process::Output, std::io::Error>
            ));

        injector
            .when_called_async(injectorpp::async_func!(
                lnet_stats_output(), Result<std::process::Output, std::io::Error>
            ))
            .will_return_async(injectorpp::async_return!(
                Ok(Output {
                    status: ExitStatus::from_raw(0),
                    stdout: include_str!("../fixtures/lnetctl_stats.txt").as_bytes().to_vec(),
                    stderr: b"".to_vec(),
                }),
                Result<std::process::Output, std::io::Error>
            ));
    }

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

    #[tokio::test(flavor = "multi_thread")]
    #[cfg(debug_assertions)]
    async fn test_metrics_endpoint_is_idempotent() -> Result<(), Box<dyn std::error::Error>> {
        let mut injector = InjectorPP::new();

        mock_command_calls(&mut injector);

        let (request, app) = get_app();

        let resp = app.oneshot(request).await?;

        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let original_body_str = std::str::from_utf8(&body).unwrap();

        let (request, app) = get_app();

        let resp = app.oneshot(request).await.unwrap();

        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let body_str = std::str::from_utf8(&body).unwrap();

        assert_eq!(
            original_body_str, body_str,
            "Stats not the same after second scrape"
        );

        insta::assert_snapshot!(original_body_str);

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    #[cfg(debug_assertions)]
    async fn test_app_function() {
        let mut injector = InjectorPP::new();

        mock_command_calls(&mut injector);

        let (request, app) = get_app();

        let response = app.oneshot(request).await.unwrap();

        assert!(response.status().is_success())
    }

    #[tokio::test(flavor = "multi_thread")]
    #[cfg(debug_assertions)]
    async fn test_app_routes() {
        let mut injector = InjectorPP::new();

        mock_command_calls(&mut injector);

        let app = crate::routes::app();

        // Test that the /metrics route exists
        let request = Request::builder()
            .uri("/metrics")
            .method("GET")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        // The route should exist
        assert!(response.status().is_success());

        // Test non-existent route returns 404
        let request = Request::builder()
            .uri("/nonexistent")
            .method("GET")
            .body(Body::empty())
            .unwrap();

        let app = crate::routes::app();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), axum::http::StatusCode::NOT_FOUND);
    }

    #[tokio::test(flavor = "multi_thread")]
    #[cfg(debug_assertions)]
    async fn test_concurrent_requests() {
        let mut injector = InjectorPP::new();

        mock_command_calls(&mut injector);

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
        assert!(result.is_ok(), "/metrics endpoint encountered a panic");
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
        let generic_error = Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            "some random error",
        )) as BoxError;
        let response = handle_error(generic_error).await.into_response();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let body_str = std::str::from_utf8(&body).unwrap();

        assert!(body_str.starts_with("Unhandled internal error:"));
    }

    #[test]
    fn test_jobstats_metrics_cmd_with_mock() -> Result<(), Box<dyn std::error::Error>> {
        setup_env();

        let mut child = crate::routes::jobstats_metrics_cmd()?;

        let mut reader = BufReader::with_capacity(
            128 * 1_024,
            child.stdout.take().ok_or(io::Error::new(
                io::ErrorKind::NotFound,
                "stdout missing for lctl jobstats call.",
            ))?,
        );

        let mut buff = String::new();
        reader.read_to_string(&mut buff)?;

        insta::assert_snapshot!(buff);

        Ok(())
    }

    #[tokio::test]
    async fn test_lustre_metrics_output_with_mock() -> Result<(), Box<dyn std::error::Error>> {
        setup_env();

        let output = crate::routes::lustre_metrics_output().await?;

        insta::assert_snapshot!(String::from_utf8(output.stdout)?);

        Ok(())
    }

    #[tokio::test]
    async fn test_net_show_output_with_mock() -> Result<(), Box<dyn std::error::Error>> {
        setup_env();

        let output = crate::routes::net_show_output().await?;

        insta::assert_snapshot!(String::from_utf8(output.stdout)?);

        Ok(())
    }

    #[tokio::test]
    async fn test_lnet_stats_output_with_mock() -> Result<(), Box<dyn std::error::Error>> {
        setup_env();

        let output = crate::routes::lnet_stats_output().await?;

        insta::assert_snapshot!(String::from_utf8(output.stdout)?);

        Ok(())
    }
}

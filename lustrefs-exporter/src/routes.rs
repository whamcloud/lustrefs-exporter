// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    Error,
    jobstats::{JobstatMetrics, jobstats_stream},
    openmetrics::{self, Metrics},
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
use lustre_collector::{parse_lctl_output, parse_lnetctl_output, parse_lnetctl_stats, parser};
use prometheus_client::{encoding::text::encode, registry::Registry};
use serde::Deserialize;
use std::{
    borrow::Cow,
    io::{self, BufRead as _, BufReader},
};
use tokio::process::Command;
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
                        tracing::debug!("stderr: {}", line);
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

    let lnetctl = net_show_output().output().await?;

    let lnetctl_stats = std::str::from_utf8(&lnetctl.stdout)?;

    let mut lnetctl_output = parse_lnetctl_output(lnetctl_stats)?;

    output.append(&mut lnetctl_output);

    let lnetctl_stats_output = lnet_stats_output().output().await?;

    let mut lnetctl_stats_record =
        parse_lnetctl_stats(std::str::from_utf8(&lnetctl_stats_output.stdout)?)?;

    output.append(&mut lnetctl_stats_record);

    // Build and register Lustre metrics
    openmetrics::build_lustre_stats(&output, &mut opentelemetry_metrics);
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
    use crate::{
        JobstatsMock, LustreMock, create_mock_commander,
        routes::{jobstats_metrics_cmd, lnet_stats_output, lustre_metrics_output, net_show_output},
    };
    use axum::{
        Router,
        body::{Body, to_bytes},
        extract::Request,
    };
    use commandeer_test::{commandeer, Commandeer, Mode};
    use sealed_test::prelude::*;
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

    #[test]
    #[serial]
    fn with_macro_test() {
        // Macro roughly_expands_to:
        let commandeer = Commandeer::new("test_with_macro_test.json", Mode::Record);
        commandeer.mock_command("git");
        // commandeer.mock_command("npm");
        // commandeer.mock_command("curl");

        let output = std::process::Command::new("git")
            .args(&["status"])
            .output()
            .unwrap();

        //assert!(output.status.success());
    }

    #[tokio::test]
    #[serial]
    async fn test_metrics_endpoint_is_idempotent() {
        let commandeer = Commandeer::new("test_metrics_endpoint_is_idempotent.json", Mode::Replay);

        commandeer.mock_command("lctl");
        commandeer.mock_command("lnetctl");



        // let _mock_commander =
        //     create_mock_commander(JobstatsMock::Lustre2_14_0_162, LustreMock::default());

        // let (request, app) = get_app();

        // let resp = app.oneshot(request).await.unwrap();

        // let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        // let original_body_str = std::str::from_utf8(&body).unwrap();

        // let (request, app) = get_app();

        // let resp = app.oneshot(request).await.unwrap();

        // let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        // let body_str = std::str::from_utf8(&body).unwrap();

        // assert_eq!(
        //     original_body_str, body_str,
        //     "Stats not the same after second scrape"
        // );

        // insta::assert_snapshot!("metrics_endpoint_is_idempotent", original_body_str);
    }

    #[sealed_test]
    fn test_app_function() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let _mock_commander =
                create_mock_commander(JobstatsMock::default(), LustreMock::default());

            let (request, app) = get_app();

            let response = app.oneshot(request).await.unwrap();

            assert!(response.status().is_success());
        });
    }

    #[sealed_test]
    fn test_app_routes() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let _mock_commander =
                create_mock_commander(JobstatsMock::default(), LustreMock::default());

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
        });
    }

    #[sealed_test]
    fn test_concurrent_requests() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let _mock_commander =
                create_mock_commander(JobstatsMock::default(), LustreMock::default());

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
        });
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

    #[sealed_test]
    fn test_jobstats_metrics_cmd_with_mock() {
        let _mock_commander =
            create_mock_commander(JobstatsMock::Lustre2_14_0_162, LustreMock::default());

        let mut child = jobstats_metrics_cmd().spawn().unwrap();

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

        insta::assert_snapshot!("jobstats_metrics_cmd_with_mock", buff);
    }

    #[sealed_test]
    fn test_lustre_metrics_output_with_mock() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let _mock_commander =
                create_mock_commander(JobstatsMock::default(), LustreMock::default());

            let output = lustre_metrics_output().output().await.unwrap();

            insta::assert_snapshot!(
                "lustre_metrics_output_with_mock",
                String::from_utf8(output.stdout).unwrap()
            );
        });
    }

    #[sealed_test]
    fn test_net_show_output_with_mock() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let _mock_commander =
                create_mock_commander(JobstatsMock::default(), LustreMock::default());

            let output = net_show_output().output().await.unwrap();

            insta::assert_snapshot!(
                "net_show_output_with_mock",
                String::from_utf8(output.stdout).unwrap()
            );
        });
    }

    #[sealed_test]
    fn test_lnet_stats_output_with_mock() {
        tokio::runtime::Runtime::new().unwrap().block_on(async {
            let _mock_commander =
                create_mock_commander(JobstatsMock::default(), LustreMock::default());

            let output = lnet_stats_output().output().await.unwrap();

            insta::assert_snapshot!(
                "lnet_stats_output_with_mock",
                String::from_utf8(output.stdout).unwrap()
            );
        });
    }
}

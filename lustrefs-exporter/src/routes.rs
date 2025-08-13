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
mod tests {
    use std::{
        os::unix::process::ExitStatusExt as _,
        process::{ExitStatus, Output},
    };

    use crate::routes::{
        jobstats_metrics_cmd, lnet_stats_output, lustre_metrics_output, net_show_output,
    };
    use axum::{
        Router,
        body::{Body, to_bytes},
        extract::Request,
    };
    use injectorpp::interface::injector::*;
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

    #[tokio::test(flavor = "multi_thread")]
    #[cfg(debug_assertions)]
    async fn test_metrics_endpoint() -> Result<(), Box<dyn std::error::Error>> {
        let mut injector = InjectorPP::new();
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
}

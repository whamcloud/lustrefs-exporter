// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    Error, init_opentelemetry,
    jobstats::opentelemetry::OpenTelemetryMetricsJobstats,
    openmetrics::{self, OpenTelemetryMetrics},
    remote_cmd::{RemoteCmd, RemoteCmdAsync},
};
use axum::{
    BoxError, Router,
    body::Body,
    error_handling::HandleErrorLayer,
    extract::{Query, State},
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

pub trait RemoteCmdRunner: RemoteCmd + RemoteCmdAsync + Send + Sync {}
impl<T: RemoteCmd + RemoteCmdAsync + Send + Sync> RemoteCmdRunner for T {}

pub struct AppState {
    pub cmd_hdl: Arc<dyn RemoteCmdRunner>,
}

pub fn app(app_state: Arc<AppState>) -> Router {
    let load_shedder = ServiceBuilder::new()
        .layer(HandleErrorLayer::new(handle_error))
        .load_shed()
        .concurrency_limit(10); // Max 10 concurrent scrape

    Router::new()
        .route("/metrics", get(scrape))
        .with_state(app_state)
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

pub async fn scrape(
    Query(params): Query<Params>,
    State(state): State<Arc<AppState>>,
) -> Result<Response<Body>, Error> {
    let (provider, registry) = init_opentelemetry()?;

    let meter = provider.meter("lustre");
    let jobstats = if params.jobstats {
        let state2 = state.clone();
        let child = tokio::task::spawn_blocking(move || {
            let mut child = std::process::Command::new("lctl");

            child
                .arg("get_param")
                .args(["obdfilter.*OST*.job_stats", "mdt.*.job_stats"])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped());

            let c = RemoteCmd::spawn(&*state2.cmd_hdl, &mut child)?;

            Ok::<_, Error>(c)
        })
        .await?;

        match child {
            Ok(mut child) => {
                let reader = BufReader::with_capacity(
                    128 * 1_024,
                    child.stdout().ok_or(io::Error::new(
                        io::ErrorKind::NotFound,
                        "stdout missing for lctl jobstats call.",
                    ))?,
                );

                let reader_stderr = BufReader::new(child.stderr().ok_or(io::Error::new(
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

    let mut lctl = Command::new("lctl");

    lctl.arg("get_param")
        .args(parser::params())
        .kill_on_drop(true);

    let lctl = RemoteCmdAsync::output(&*state.cmd_hdl, &mut lctl).await?;

    let mut lctl_output = parse_lctl_output(&lctl.stdout)?;

    output.append(&mut lctl_output);

    let mut lnetctl = Command::new("lnetctl");

    lnetctl.args(["net", "show", "-v", "4"]).kill_on_drop(true);

    let lnetctl = RemoteCmdAsync::output(&*state.cmd_hdl, &mut lnetctl).await?;

    let lnetctl_stats = std::str::from_utf8(&lnetctl.stdout)?;
    let mut lnetctl_output = parse_lnetctl_output(lnetctl_stats)?;

    output.append(&mut lnetctl_output);

    let mut lnetctl_stats_output = Command::new("lnetctl");

    lnetctl_stats_output
        .args(["stats", "show"])
        .kill_on_drop(true);

    let lnetctl_stats_output =
        RemoteCmdAsync::output(&*state.cmd_hdl, &mut lnetctl_stats_output).await?;

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

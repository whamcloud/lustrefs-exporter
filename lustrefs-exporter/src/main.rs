// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use axum::{
    BoxError, Router,
    body::Body,
    error_handling::HandleErrorLayer,
    extract::Query,
    http::{self, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
};
use clap::Parser;
use lustre_collector::{parse_lctl_output, parse_lnetctl_output, parse_lnetctl_stats, parser};
use lustrefs_exporter::{
    Error,
    jobstats::opentelemetry::OpenTelemetryMetricsJobstats,
    openmetrics::{self, OpenTelemetryMetrics, init_opentelemetry},
};
use opentelemetry::metrics::MeterProvider;
use prometheus::{Encoder as _, TextEncoder};
use serde::Deserialize;
use std::{
    borrow::Cow,
    convert::Infallible,
    io::{self, BufRead, BufReader},
    net::SocketAddr,
    sync::Arc,
};
use tokio::process::Command;
use tokio_stream::StreamExt as _;
use tower::ServiceBuilder;

const LUSTREFS_EXPORTER_PORT: &str = "32221";

#[derive(Debug, Parser)]
pub struct CommandOpts {
    /// Port that exporter will listen to
    #[clap(short, long, env = "LUSTREFS_EXPORTER_PORT", default_value = LUSTREFS_EXPORTER_PORT)]
    pub port: u16,

    /// Dump stats as raw string and exit
    #[clap(long, hide = true)]
    pub dump: bool,
}

async fn handle_error(error: BoxError) -> impl IntoResponse {
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

#[derive(Debug, Deserialize)]
struct Params {
    // Only enable jobstats if "jobstats=true"
    #[serde(default)]
    jobstats: bool,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();

    let opts = CommandOpts::parse();

    if opts.dump {
        println!("# Dumping lctl get_param output");
        let lctl = Command::new("lctl")
            .arg("get_param")
            .args(parser::params())
            .kill_on_drop(true)
            .output()
            .await?;
        println!("{}", std::str::from_utf8(&lctl.stdout)?);

        println!("# Dumping lctl get_param jobstats output");
        let lctl = Command::new("lctl")
            .arg("get_param")
            .args(["obdfilter.*OST*.job_stats", "mdt.*.job_stats"])
            .kill_on_drop(true)
            .output()
            .await?;
        println!("{}", std::str::from_utf8(&lctl.stdout)?);

        println!("# Dumping lnetctl net show output");
        let lnetctl = Command::new("lnetctl")
            .args(["net", "show", "-v", "4"])
            .kill_on_drop(true)
            .output()
            .await?;

        println!("{}", std::str::from_utf8(&lnetctl.stdout)?);

        println!("# Dumping lnetctl stats show output");
        let lnetctl_stats_output = Command::new("lnetctl")
            .args(["stats", "show"])
            .kill_on_drop(true)
            .output()
            .await?;
        println!("{}", std::str::from_utf8(&lnetctl_stats_output.stdout)?);
    } else {
        let addr = SocketAddr::from(([0, 0, 0, 0], opts.port));

        tracing::info!("Listening on http://{addr}/metrics");

        let listener = tokio::net::TcpListener::bind(("0.0.0.0", opts.port)).await?;

        let load_shedder = ServiceBuilder::new()
            .layer(HandleErrorLayer::new(handle_error))
            .load_shed()
            .concurrency_limit(10); // Max 10 concurrent scrape

        let mut app = Router::new()
            .route("/metrics", get(scrape))
            .layer(load_shedder);

        #[cfg(target_arch = "x86_64")]
        {
            use lustrefs_exporter::profiling;

            // Enable heap profiling for x86_64
            app = app.route("/heap", get(profiling::handle_get_heap));
        }

        axum::serve(listener, app).await?;
    }

    Ok(())
}

async fn scrape(Query(params): Query<Params>) -> Result<Response<Body>, Error> {
    let (provider, registry) = init_opentelemetry()?;

    let meter = provider.meter("lustre");
    let jobstats = if params.jobstats {
        let child = tokio::task::spawn_blocking(move || {
            let child = std::process::Command::new("lctl")
                .arg("get_param")
                .args(["obdfilter.*OST*.job_stats", "mdt.*.job_stats"])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()?;

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

                let handle = lustrefs_exporter::jobstats::opentelemetry::jobstats_stream(
                    reader,
                    otel_jobstats.clone(),
                );

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

    let lctl = Command::new("lctl")
        .arg("get_param")
        .args(parser::params())
        .kill_on_drop(true)
        .output()
        .await?;

    let mut lctl_output = parse_lctl_output(&lctl.stdout)?;

    output.append(&mut lctl_output);

    let lnetctl = Command::new("lnetctl")
        .args(["net", "show", "-v", "4"])
        .kill_on_drop(true)
        .output()
        .await?;

    let lnetctl_stats = std::str::from_utf8(&lnetctl.stdout)?;
    let mut lnetctl_output = parse_lnetctl_output(lnetctl_stats)?;

    output.append(&mut lnetctl_output);

    let lnetctl_stats_output = Command::new("lnetctl")
        .args(["stats", "show"])
        .kill_on_drop(true)
        .output()
        .await?;

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

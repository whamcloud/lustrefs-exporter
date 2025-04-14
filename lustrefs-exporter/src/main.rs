// Copyright (c) 2024 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use axum::{
    body::{Body, Bytes},
    error_handling::HandleErrorLayer,
    extract::Query,
    http::{self, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::get,
    BoxError, Extension, Router,
};
use clap::Parser;
use lustre_collector::{parse_lctl_output, parse_lnetctl_output, parse_lnetctl_stats, parser};
use lustrefs_exporter::{
    build_lustre_stats,
    openmetrics::{self, OpenTelemetryMetrics},
    Error,
};
use opentelemetry::{
    global,
    metrics::{Meter, MeterProvider},
};
use opentelemetry_sdk::{metrics::SdkMeterProvider, Resource};
use prometheus::{Encoder as _, Registry, TextEncoder};
use serde::Deserialize;
use std::{
    borrow::Cow,
    convert::Infallible,
    io::{self, BufRead, BufReader},
    net::SocketAddr,
};
use tokio::process::Command;
use tokio_stream::{wrappers::ReceiverStream, StreamExt};
use tower::ServiceBuilder;

const LUSTREFS_EXPORTER_PORT: &str = "32221";

#[derive(Debug, Parser)]
pub struct CommandOpts {
    /// Port that exporter will listen to
    #[clap(short, long, env = "LUSTREFS_EXPORTER_PORT", default_value = LUSTREFS_EXPORTER_PORT)]
    pub port: u16,
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

pub fn init_opentelemetry() -> Result<
    (opentelemetry_sdk::metrics::SdkMeterProvider, Registry),
    opentelemetry_sdk::metrics::MetricError,
> {
    // Build the Prometheus exporter.
    // The metrics will be exposed in the Prometheus format.
    let registry = Registry::new();
    let prometheus_exporter = opentelemetry_prometheus::exporter()
        .with_registry(registry.clone())
        .without_counter_suffixes()
        .build()?;

    let provider = SdkMeterProvider::builder()
        .with_reader(prometheus_exporter)
        .with_resource(
            Resource::builder()
                .with_service_name("lustrefs-exporter")
                .build(),
        )
        .build();

    Ok((provider, registry))
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();

    let opts = CommandOpts::parse();

    let (provider, registry) = init_opentelemetry()?;

    // Set the global MeterProvider to the one created above.
    // This will make all meters created with `global::meter()` use the above MeterProvider.
    global::set_meter_provider(provider.clone());

    let meter = provider.meter("lustre");

    let addr = SocketAddr::from(([0, 0, 0, 0], opts.port));

    tracing::info!("Listening on http://{addr}/metrics");

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", opts.port)).await?;

    let load_shedder = ServiceBuilder::new()
        .layer(HandleErrorLayer::new(handle_error))
        .load_shed()
        .concurrency_limit(10); // Max 10 concurrent scrape

    let app = Router::new()
        .route("/metrics", get(scrape))
        .layer(load_shedder)
        .layer(Extension(registry.clone()))
        .layer(Extension(meter.clone()));

    axum::serve(listener, app).await?;

    Ok(())
}

async fn scrape(
    Query(params): Query<Params>,
    Extension(registry): Extension<Registry>,
    Extension(meter): Extension<Meter>,
) -> Result<Response<Body>, Error> {
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

                let (_, rx) = lustrefs_exporter::jobstats::jobstats_stream(reader);

                tokio::task::spawn_blocking(move || {
                    if let Err(e) = child.wait() {
                        tracing::debug!("Unexpected error when waiting for child: {e}");
                    }
                });

                let stream = ReceiverStream::new(rx)
                    .map(|x| Bytes::from_iter(x.into_bytes()))
                    .map(Ok::<_, Infallible>);

                Some(stream)
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

    let mut buffer = vec![];
    let encoder = TextEncoder::new();
    let metric_families = registry.gather();
    let encoder_results = encoder.encode(&metric_families, &mut buffer);

    // Build OTEL metrics
    openmetrics::build_lustre_stats(&output, opentelemetry_metrics);

    let lustre_stats = build_lustre_stats(output);

    let body = if let Some(stream) = jobstats {
        let merged =
            tokio_stream::StreamExt::chain(tokio_stream::once(Ok(lustre_stats.into())), stream);

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
    use crate::{build_lustre_stats, init_opentelemetry};
    use combine::parser::EasyParser;
    use include_dir::{include_dir, Dir};
    use insta::assert_snapshot;
    use lustre_collector::parser::parse;
    use lustrefs_exporter::openmetrics::OpenTelemetryMetrics;
    use opentelemetry::metrics::MeterProvider;
    use prometheus::{Encoder as _, TextEncoder};

    static VALID_FIXTURES: Dir<'_> =
        include_dir!("$CARGO_MANIFEST_DIR/../lustre-collector/src/fixtures/valid/");

    #[test]
    fn test_valid_fixtures() {
        for dir in VALID_FIXTURES.find("*").unwrap() {
            match dir {
                include_dir::DirEntry::Dir(_) => {}
                include_dir::DirEntry::File(file) => {
                    let name = file.path().to_string_lossy();

                    let contents = file.contents_utf8().unwrap();

                    let result = parse()
                        .easy_parse(contents)
                        .map_err(|err| err.map_position(|p| p.translate_position(contents)))
                        .unwrap();

                    let x = build_lustre_stats(result.0);

                    assert_snapshot!(format!("valid_fixture_{name}"), x);
                }
            }
        }
    }

    #[test]
    fn test_valid_fixtures_otel() {
        for dir in VALID_FIXTURES.find("*").unwrap() {
            match dir {
                include_dir::DirEntry::Dir(_) => {}
                include_dir::DirEntry::File(file) => {
                    let name = file.path().to_string_lossy();

                    let contents = file.contents_utf8().unwrap();

                    let result = parse()
                        .easy_parse(contents)
                        .map_err(|err| err.map_position(|p| p.translate_position(contents)))
                        .unwrap();

                    let (provider, registry) =
                        init_opentelemetry().expect("Failed to initialize OpenTelemetry");

                    let meter = provider.meter("lustre");

                    let otel = OpenTelemetryMetrics::new(meter);

                    crate::openmetrics::build_lustre_stats(&result.0, otel);

                    let mut buffer = vec![];
                    let encoder = TextEncoder::new();
                    let metric_families = registry.gather();
                    encoder.encode(&metric_families, &mut buffer).unwrap();

                    let x = String::from_utf8_lossy(&buffer).to_string();

                    assert_snapshot!(format!("valid_fixture_otel_{name}"), x);
                }
            }
        }
    }

    #[test]
    fn test_stats() {
        let output = include_str!("../fixtures/stats.json");

        let x = serde_json::from_str(output).unwrap();

        let x = build_lustre_stats(x);

        insta::assert_snapshot!(x);
    }

    #[test]
    fn test_lnetctl_stats() {
        let output = include_str!("../fixtures/lnetctl_stats.json");

        let x = serde_json::from_str(output).unwrap();

        let x = build_lustre_stats(x);

        insta::assert_snapshot!(x);
    }

    #[test]
    fn test_lnetctl_stats_mds() {
        let output = include_str!("../fixtures/stats_mds.json");

        let x = serde_json::from_str(output).unwrap();

        let x = build_lustre_stats(x);

        insta::assert_snapshot!(x);
    }

    #[test]
    fn test_host_stats_non_healthy() {
        let output = include_str!("../fixtures/host_stats_non_healthy.json");

        let x = serde_json::from_str(output).unwrap();

        let x = build_lustre_stats(x);

        insta::assert_snapshot!(x);
    }

    #[test]
    fn test_client_stats() {
        let output = include_str!("../fixtures/client.json");

        let x = serde_json::from_str(output).unwrap();

        let x = build_lustre_stats(x);

        insta::assert_snapshot!(x);
    }
}

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
    Error, init_opentelemetry,
    jobstats::opentelemetry::OpenTelemetryMetricsJobstats,
    openmetrics::{self, OpenTelemetryMetrics},
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

        let app = Router::new()
            .route("/metrics", get(scrape))
            .layer(load_shedder);

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

#[cfg(test)]
mod tests {
    use crate::init_opentelemetry;
    use combine::parser::EasyParser;
    use include_dir::{Dir, include_dir};
    use insta::assert_snapshot;
    use lustre_collector::parser::parse;
    use lustrefs_exporter::openmetrics::OpenTelemetryMetrics;
    use opentelemetry::metrics::MeterProvider;
    use prometheus::{Encoder as _, Registry, TextEncoder};
    use prometheus_parse::{Sample, Scrape};
    use std::{collections::HashSet, error::Error, fs};

    static VALID_FIXTURES: Dir<'_> =
        include_dir!("$CARGO_MANIFEST_DIR/../lustre-collector/src/fixtures/valid/");

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
    fn test_stats_otel() {
        let output = include_str!("../fixtures/stats.json");

        let x = serde_json::from_str(output).unwrap();

        let (provider, registry) =
            init_opentelemetry().expect("Failed to initialize OpenTelemetry");

        let meter = provider.meter("lustre");

        let otel = OpenTelemetryMetrics::new(meter);

        crate::openmetrics::build_lustre_stats(&x, otel);

        insta::assert_snapshot!(get_output(&registry));

        let opentelemetry = read_metrics_from_snapshot(
            format!(
                "{}/src/snapshots/lustrefs_exporter__tests__stats_otel.snap",
                env!("CARGO_MANIFEST_DIR")
            )
            .as_str(),
        );
        let previous_implementation = read_metrics_from_snapshot(
            format!(
                "{}/src/snapshots/lustrefs_exporter__tests__stats.snap",
                env!("CARGO_MANIFEST_DIR")
            )
            .as_str(),
        );
        compare_metrics(&opentelemetry.unwrap(), &previous_implementation.unwrap());
    }

    #[test]
    fn test_lnetctl_stats_otel() {
        let output = include_str!("../fixtures/lnetctl_stats.json");

        let x = serde_json::from_str(output).unwrap();

        let (provider, registry) =
            init_opentelemetry().expect("Failed to initialize OpenTelemetry");

        let meter = provider.meter("lustre");

        let otel = OpenTelemetryMetrics::new(meter);

        crate::openmetrics::build_lustre_stats(&x, otel);

        insta::assert_snapshot!(get_output(&registry));

        let opentelemetry = read_metrics_from_snapshot(
            format!(
                "{}/src/snapshots/lustrefs_exporter__tests__lnetctl_stats_otel.snap",
                env!("CARGO_MANIFEST_DIR")
            )
            .as_str(),
        );
        let previous_implementation = read_metrics_from_snapshot(
            format!(
                "{}/src/snapshots/lustrefs_exporter__tests__lnetctl_stats.snap",
                env!("CARGO_MANIFEST_DIR")
            )
            .as_str(),
        );
        compare_metrics(&opentelemetry.unwrap(), &previous_implementation.unwrap());
    }

    #[test]
    fn test_lnetctl_stats_mds_otel() {
        let output = include_str!("../fixtures/stats_mds.json");

        let x = serde_json::from_str(output).unwrap();

        let (provider, registry) =
            init_opentelemetry().expect("Failed to initialize OpenTelemetry");

        let meter = provider.meter("lustre");

        let otel = OpenTelemetryMetrics::new(meter);

        crate::openmetrics::build_lustre_stats(&x, otel);

        insta::assert_snapshot!(get_output(&registry));

        let opentelemetry = read_metrics_from_snapshot(
            format!(
                "{}/src/snapshots/lustrefs_exporter__tests__lnetctl_stats_mds_otel.snap",
                env!("CARGO_MANIFEST_DIR")
            )
            .as_str(),
        );
        let previous_implementation = read_metrics_from_snapshot(
            format!(
                "{}/src/snapshots/lustrefs_exporter__tests__lnetctl_stats_mds.snap",
                env!("CARGO_MANIFEST_DIR")
            )
            .as_str(),
        );
        compare_metrics(&opentelemetry.unwrap(), &previous_implementation.unwrap());
    }

    #[test]
    fn test_host_stats_non_healthy_otel() {
        let output = include_str!("../fixtures/host_stats_non_healthy.json");

        let x = serde_json::from_str(output).unwrap();

        let (provider, registry) =
            init_opentelemetry().expect("Failed to initialize OpenTelemetry");

        let meter = provider.meter("lustre");

        let otel = OpenTelemetryMetrics::new(meter);

        crate::openmetrics::build_lustre_stats(&x, otel);

        insta::assert_snapshot!(get_output(&registry));

        let opentelemetry = read_metrics_from_snapshot(
            format!(
                "{}/src/snapshots/lustrefs_exporter__tests__host_stats_non_healthy_otel.snap",
                env!("CARGO_MANIFEST_DIR")
            )
            .as_str(),
        );
        let previous_implementation = read_metrics_from_snapshot(
            format!(
                "{}/src/snapshots/lustrefs_exporter__tests__host_stats_non_healthy.snap",
                env!("CARGO_MANIFEST_DIR")
            )
            .as_str(),
        );
        compare_metrics(&opentelemetry.unwrap(), &previous_implementation.unwrap());
    }

    #[test]
    fn test_client_stats_otel() {
        let output = include_str!("../fixtures/client.json");

        let x = serde_json::from_str(output).unwrap();

        let (provider, registry) =
            init_opentelemetry().expect("Failed to initialize OpenTelemetry");

        let meter = provider.meter("lustre");

        let otel = OpenTelemetryMetrics::new(meter);

        crate::openmetrics::build_lustre_stats(&x, otel);

        insta::assert_snapshot!(get_output(&registry));

        let opentelemetry = read_metrics_from_snapshot(
            format!(
                "{}/src/snapshots/lustrefs_exporter__tests__client_stats_otel.snap",
                env!("CARGO_MANIFEST_DIR")
            )
            .as_str(),
        );
        let previous_implementation = read_metrics_from_snapshot(
            format!(
                "{}/src/snapshots/lustrefs_exporter__tests__client_stats.snap",
                env!("CARGO_MANIFEST_DIR")
            )
            .as_str(),
        );
        compare_metrics(&opentelemetry.unwrap(), &previous_implementation.unwrap());
    }
    use pretty_assertions::assert_eq;

    // Make sure metrics from the OpenTelemetry implementation are the same as the previous implementation
    #[tokio::test]
    async fn test_legacy_metrics() -> Result<(), Box<dyn std::error::Error>> {
        // Generate snapshots for the OpenTelemetry implementation
        test_valid_fixtures_otel();

        // Compare snapshots
        for dir in VALID_FIXTURES.find("*").unwrap() {
            match dir {
                include_dir::DirEntry::Dir(_) => {}
                include_dir::DirEntry::File(file) => {
                    let name = file.path().to_string_lossy().to_string().replace("/", "__");
                    // Useful when debugging
                    // println!("{}", format!("{}/src/snapshots/lustrefs_exporter__tests__valid_fixture_otel_{name}.snap", env!("CARGO_MANIFEST_DIR")));
                    let opentelemetry = read_metrics_from_snapshot(format!("{}/src/snapshots/lustrefs_exporter__tests__valid_fixture_otel_{name}.snap", env!("CARGO_MANIFEST_DIR")).as_str());
                    let previous_implementation = read_metrics_from_snapshot(
                        format!(
                            "{}/src/snapshots/lustrefs_exporter__tests__valid_fixture_{name}.snap",
                            env!("CARGO_MANIFEST_DIR")
                        )
                        .as_str(),
                    );
                    compare_metrics(&opentelemetry.unwrap(), &previous_implementation.unwrap());
                }
            }
        }
        Ok(())
    }

    fn read_metrics_from_snapshot(path: &str) -> Result<Scrape, Box<dyn Error>> {
        let content = fs::read_to_string(path)?;

        // Skip insta header
        let content = content
            .lines()
            .skip(4)
            .map(|s| Ok(s.to_owned()))
            .collect::<Vec<_>>();
        let parsed = Scrape::parse(content.into_iter())?;
        Ok(parsed)
    }

    fn normalize_sample(sample: &Sample) -> (String, Vec<(String, String)>, String) {
        let mut sorted_labels: Vec<_> = sample
            .labels
            .iter()
            .filter(|(k, _)| k != &&"otel_scope_name".to_string())
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        sorted_labels.sort();

        let value_str = match sample.value {
            prometheus_parse::Value::Counter(f) => format!("Counter({})", f),
            prometheus_parse::Value::Gauge(f) => format!("Gauge({})", f),
            _ => "0.0".to_string(),
        };

        (sample.metric.clone(), sorted_labels, value_str)
    }

    fn normalize_docs(docs: &std::collections::HashMap<String, String>) -> Vec<(String, String)> {
        // Ignore updated metrics since OTEL move.
        let mut sorted_docs: Vec<_> = docs
            .iter()
            .filter_map(|(k, v)| {
                if k != "target_info" && k != "lustre_health_healthy" {
                    Some((k.clone(), v.clone()))
                } else {
                    None
                }
            })
            .collect();
        sorted_docs.sort_by(|a, b| a.0.cmp(&b.0)); // Sort by key
        sorted_docs
    }

    fn compare_metrics(metrics1: &Scrape, metrics2: &Scrape) {
        // Skip OTEL specific metric and updated metrics.
        let set1: HashSet<_> = metrics1
            .samples
            .iter()
            .filter(|s| s.metric != "target_info")
            .map(normalize_sample)
            .collect();
        let set2: HashSet<_> = metrics2
            .samples
            .iter()
            .filter(|s| s.metric != "target_info")
            .map(normalize_sample)
            .collect();

        let only_in_first: Vec<_> = set1.difference(&set2).collect();
        let only_in_second: Vec<_> = set2.difference(&set1).collect();

        let metric_value_comparison = if only_in_first.is_empty() && only_in_second.is_empty() {
            true
        } else {
            if !only_in_first.is_empty() {
                println!("Metrics only in first file:");
                for metric in only_in_first {
                    println!("{:?}", metric);
                }
            }
            if !only_in_second.is_empty() {
                println!("Metrics only in second file:");
                for metric in only_in_second {
                    println!("{:?}", metric);
                }
            }
            false
        };

        // Assert metrics values/labels are exactly the same
        assert!(
            metric_value_comparison,
            "Metrics values/labels are not the same"
        );

        // Normalize and compare metrics help
        let normalized_docs1 = normalize_docs(&metrics1.docs);
        let normalized_docs2 = normalize_docs(&metrics2.docs);

        assert_eq!(
            normalized_docs1, normalized_docs2,
            "Metrics help are not the same"
        );
    }

    fn get_output(registry: &Registry) -> String {
        let encoder = TextEncoder::new();
        let mut output = Vec::new();
        encoder.encode(&registry.gather(), &mut output).unwrap();
        String::from_utf8(output).unwrap()
    }
}

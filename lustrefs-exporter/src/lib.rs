// Copyright (c) 2024 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

pub mod brw_stats;
pub mod host;
pub mod jobstats;
pub mod llite;
pub mod lnet;
pub mod openmetrics;
pub mod quota;
pub mod routes;
pub mod service;
pub mod stats;

use axum::{
    http::{self, StatusCode},
    response::{IntoResponse, Response},
};
use lustre_collector::{LustreCollectorError, TargetVariant};
use opentelemetry_sdk::{Resource, metrics::SdkMeterProvider};
use prometheus::Registry;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Http(#[from] http::Error),
    #[error(transparent)]
    TaskJoin(#[from] tokio::task::JoinError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    LustreCollector(#[from] LustreCollectorError),
    #[error(transparent)]
    Utf8(#[from] std::str::Utf8Error),
    #[error("Could not find match for {0} in {1}")]
    NoCap(&'static str, String),
    #[error(transparent)]
    Otel(#[from] opentelemetry_sdk::metrics::MetricError),
    #[error(transparent)]
    Prometheus(#[from] prometheus::Error),
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        tracing::warn!("{self}");

        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

trait LabelProm {
    fn to_prom_label(&self) -> &'static str;
}

impl LabelProm for TargetVariant {
    fn to_prom_label(&self) -> &'static str {
        match self {
            TargetVariant::Ost => "ost",
            TargetVariant::Mgt => "mgt",
            TargetVariant::Mdt => "mdt",
        }
    }
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

#[cfg(test)]
pub mod tests {
    use crate::{
        init_opentelemetry,
        openmetrics::{self, OpenTelemetryMetrics},
    };
    use combine::EasyParser as _;
    use lustre_collector::parser::parse;
    use opentelemetry::metrics::MeterProvider as _;
    use prometheus::{Encoder as _, Registry, TextEncoder};
    use prometheus_parse::{Sample, Scrape};
    use std::{
        collections::HashSet,
        error::Error,
        path::{Path, PathBuf},
    };

    #[test]
    fn test_stats_otel() {
        let output = include_str!("../fixtures/stats.json");

        let x = serde_json::from_str(output).unwrap();

        let (provider, registry) =
            init_opentelemetry().expect("Failed to initialize OpenTelemetry");

        let meter = provider.meter("lustre");

        let otel = OpenTelemetryMetrics::new(meter);

        openmetrics::build_lustre_stats(&x, otel);

        let stats = get_output(&registry);

        insta::assert_snapshot!(stats);

        let current = get_scrape(stats).unwrap();

        let previous = read_metrics_from_snapshot(&historical_snapshot_path(
            "lustrefs_exporter__tests__stats.histsnap",
        ))
        .unwrap();

        compare_metrics(&current, &previous);
    }

    #[test]
    fn test_lnetctl_stats_otel() {
        let output = include_str!("../fixtures/lnetctl_stats.json");

        let x = serde_json::from_str(output).unwrap();

        let (provider, registry) =
            init_opentelemetry().expect("Failed to initialize OpenTelemetry");

        let meter = provider.meter("lustre");

        let otel = OpenTelemetryMetrics::new(meter);

        openmetrics::build_lustre_stats(&x, otel);

        let stats = get_output(&registry);

        insta::assert_snapshot!(stats);

        let current = get_scrape(stats).unwrap();

        let previous = read_metrics_from_snapshot(&historical_snapshot_path(
            "lustrefs_exporter__tests__lnetctl_stats.histsnap",
        ))
        .unwrap();

        compare_metrics(&current, &previous);
    }

    #[test]
    fn test_lnetctl_stats_mds_otel() {
        let output = include_str!("../fixtures/stats_mds.json");

        let x = serde_json::from_str(output).unwrap();

        let (provider, registry) =
            init_opentelemetry().expect("Failed to initialize OpenTelemetry");

        let meter = provider.meter("lustre");

        let otel = OpenTelemetryMetrics::new(meter);

        openmetrics::build_lustre_stats(&x, otel);

        let stats = get_output(&registry);

        insta::assert_snapshot!(stats);

        let current = get_scrape(stats).unwrap();

        let previous = read_metrics_from_snapshot(&historical_snapshot_path(
            "lustrefs_exporter__tests__lnetctl_stats_mds.histsnap",
        ))
        .unwrap();

        compare_metrics(&current, &previous);
    }

    #[test]
    fn test_host_stats_non_healthy_otel() {
        let output = include_str!("../fixtures/host_stats_non_healthy.json");

        let x = serde_json::from_str(output).unwrap();

        let (provider, registry) =
            init_opentelemetry().expect("Failed to initialize OpenTelemetry");

        let meter = provider.meter("lustre");

        let otel = OpenTelemetryMetrics::new(meter);

        openmetrics::build_lustre_stats(&x, otel);

        let stats = get_output(&registry);

        insta::assert_snapshot!(stats);

        let current = get_scrape(stats).unwrap();

        let previous = read_metrics_from_snapshot(&historical_snapshot_path(
            "lustrefs_exporter__tests__host_stats_non_healthy.histsnap",
        ))
        .unwrap();

        compare_metrics(&current, &previous);
    }

    #[test]
    fn test_client_stats_otel() {
        let output = include_str!("../fixtures/client.json");

        let x = serde_json::from_str(output).unwrap();

        let (provider, registry) =
            init_opentelemetry().expect("Failed to initialize OpenTelemetry");

        let meter = provider.meter("lustre");

        let otel = OpenTelemetryMetrics::new(meter);

        openmetrics::build_lustre_stats(&x, otel);

        let stats = get_output(&registry);

        insta::assert_snapshot!(stats);

        let current = get_scrape(stats).unwrap();

        let previous = read_metrics_from_snapshot(&historical_snapshot_path(
            "lustrefs_exporter__tests__client_stats.histsnap",
        ))
        .unwrap();

        compare_metrics(&current, &previous);
    }

    // Make sure metrics from the OpenTelemetry implementation are the same as the previous implementation
    #[test]
    fn valid_fixture_otel() -> Result<(), Box<dyn std::error::Error>> {
        insta::glob!(
            "../../lustre-collector/src/fixtures/valid/",
            "**/*.txt",
            |path| {
                let contents = std::fs::read_to_string(path).unwrap();

                let x = parse_lustre_metrics(&contents);

                insta::assert_snapshot!(x);

                let current = Scrape::parse(x.lines().map(|x| Ok(x.to_owned()))).unwrap();

                let x = path.display().to_string();

                let (_, x) = x
                    .split_once("lustre-collector/src/fixtures/valid/")
                    .unwrap();

                let name = x.replace('/', "__");

                let name = format!("lustrefs_exporter__tests__valid_fixture_{name}.histsnap");

                let historical_snap = PathBuf::from_iter([
                    env!("CARGO_MANIFEST_DIR"),
                    "src",
                    "historical_snapshots",
                    &name,
                ]);

                let previous = read_metrics_from_snapshot(&historical_snap).unwrap();

                compare_metrics(&current, &previous);
            }
        );

        Ok(())
    }

    pub(super) fn compare_metrics(metrics1: &Scrape, metrics2: &Scrape) {
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
                    println!("{metric:?}");
                }
            }

            if !only_in_second.is_empty() {
                println!("Metrics only in second file:");

                for metric in only_in_second {
                    println!("{metric:?}");
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

        pretty_assertions::assert_eq!(
            normalized_docs1,
            normalized_docs2,
            "Metrics help are not the same"
        );
    }

    pub(super) fn historical_snapshot_path(name: &str) -> PathBuf {
        PathBuf::from_iter([
            env!("CARGO_MANIFEST_DIR"),
            "src",
            "historical_snapshots",
            name,
        ])
    }

    pub(super) fn get_scrape(x: String) -> Result<Scrape, Box<dyn Error>> {
        let x = x.lines().map(|x| Ok(x.to_owned()));

        let x = Scrape::parse(x)?;

        Ok(x)
    }

    pub(super) fn read_metrics_from_snapshot(path: &Path) -> Result<Scrape, Box<dyn Error>> {
        let x = insta::Snapshot::from_file(path).unwrap();

        let insta::internals::SnapshotContents::Text(x) = x.contents() else {
            panic!("Snapshot is not text");
        };

        let parsed = get_scrape(x.to_string())?;

        Ok(parsed)
    }

    fn parse_lustre_metrics(contents: &str) -> String {
        let result = parse()
            .easy_parse(contents)
            .map_err(|err| err.map_position(|p| p.translate_position(contents)))
            .unwrap();

        let (provider, registry) =
            init_opentelemetry().expect("Failed to initialize OpenTelemetry");

        let meter = provider.meter("lustre");

        let otel = OpenTelemetryMetrics::new(meter);

        openmetrics::build_lustre_stats(&result.0, otel);

        let mut buffer = vec![];
        let encoder = TextEncoder::new();
        let metric_families = registry.gather();
        encoder.encode(&metric_families, &mut buffer).unwrap();

        String::from_utf8_lossy(&buffer).to_string()
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
            prometheus_parse::Value::Counter(f) => format!("Counter({f})"),
            prometheus_parse::Value::Gauge(f) => format!("Gauge({f})"),
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

    fn get_output(registry: &Registry) -> String {
        let encoder = TextEncoder::new();
        let mut output = Vec::new();
        encoder.encode(&registry.gather(), &mut output).unwrap();
        String::from_utf8(output).unwrap()
    }
}

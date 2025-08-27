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
use prometheus_client::metrics::family::Family as PrometheusFamily;

pub type LabelContainer = Vec<(&'static str, String)>;
pub type Family<T> = PrometheusFamily<LabelContainer, T>;

/// Creates a label container by combining provided labels with the default OpenTelemetry scope label.
///
/// This function takes a slice of label tuples and appends a default `otel_scope_name` label
/// with the value "lustre" to create a complete set of labels for Prometheus metrics.
///
/// # Arguments
///
/// * `labels` - A slice of tuples containing label key-value pairs where keys are static strings
///   and values are owned strings
///
/// # Returns
///
/// A `LabelContainer` (vector of label tuples) containing the input labels plus the default
/// OpenTelemetry scope label
///
/// # Examples
///
/// ```
/// use lustrefs_exporter::create_labels;
///
/// let labels = create_labels(&[
///     ("component", "mdt".to_string()),
///     ("target", "fs-MDT0000".to_string()),
/// ]);
/// // Results in: [("component", "mdt"), ("target", "fs-MDT0000"), ("otel_scope_name", "lustre")]
/// ```
pub fn create_labels(labels: &[(&'static str, String)]) -> LabelContainer {
    let mut result = Vec::with_capacity(labels.len() + 1);

    result.extend_from_slice(labels);

    result.push(("otel_scope_name", "lustre".to_string()));

    result
}

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Fmt(#[from] std::fmt::Error),
    #[error(transparent)]
    Http(#[from] http::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    LustreCollector(#[from] LustreCollectorError),
    #[error("Could not find match for {0} in {1}")]
    NoCap(&'static str, String),
    #[error(transparent)]
    OneshotReceive(#[from] tokio::sync::oneshot::error::RecvError),
    #[error("{0}")]
    Prometheus(std::fmt::Error),
    #[error(transparent)]
    TaskJoin(#[from] tokio::task::JoinError),
    #[error(transparent)]
    Utf8(#[from] std::str::Utf8Error),
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

#[cfg(test)]
pub mod tests {
    use crate::{
        Error, LabelProm as _, create_labels,
        openmetrics::{self, Metrics},
    };
    use axum::{http::StatusCode, response::IntoResponse as _};
    use combine::EasyParser as _;
    use lustre_collector::{Record, TargetVariant, parser::parse};
    use prometheus_client::{encoding::text::encode, registry::Registry};
    use prometheus_parse::{Sample, Scrape};
    use std::{
        collections::HashSet,
        path::{Path, PathBuf},
    };

    #[test]
    fn test_create_labels_empty() {
        let result = create_labels(&[]);
        assert_eq!(result, vec![("otel_scope_name", "lustre".to_string())]);
    }

    #[test]
    fn test_create_labels_single() {
        let result = create_labels(&[("component", "mdt".to_string())]);
        assert_eq!(
            result,
            vec![
                ("component", "mdt".to_string()),
                ("otel_scope_name", "lustre".to_string())
            ]
        );
    }

    #[test]
    fn test_create_labels_multiple() {
        let result = create_labels(&[
            ("component", "mdt".to_string()),
            ("target", "fs-MDT0000".to_string()),
            ("operation", "read".to_string()),
        ]);
        assert_eq!(
            result,
            vec![
                ("component", "mdt".to_string()),
                ("target", "fs-MDT0000".to_string()),
                ("operation", "read".to_string()),
                ("otel_scope_name", "lustre".to_string())
            ]
        );
    }

    #[test]
    fn test_error_into_response() {
        let error = Error::NoCap("test_param", "test_content".to_string());
        let response = error.into_response();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_target_variant_to_prom_label() {
        assert_eq!(TargetVariant::Ost.to_prom_label(), "ost");
        assert_eq!(TargetVariant::Mgt.to_prom_label(), "mgt");
        assert_eq!(TargetVariant::Mdt.to_prom_label(), "mdt");
    }

    #[tokio::test(flavor = "multi_thread")]
    #[cfg(test)]
    async fn test_stats_otel() {
        let output = include_str!("../fixtures/stats.json");

        let stats = encode_lustre_stats_from_fixture(output);

        insta::assert_snapshot!(stats);

        let current = get_scrape(stats);

        let previous = read_metrics_from_snapshot(&historical_snapshot_path(
            "lustrefs_exporter__tests__stats.histsnap",
        ));

        compare_metrics(&current, &previous);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_lnetctl_stats_otel() {
        let output = include_str!("../fixtures/lnetctl_stats.json");

        let stats = encode_lustre_stats_from_fixture(output);

        insta::assert_snapshot!(stats);

        let current = get_scrape(stats);

        let previous = read_metrics_from_snapshot(&historical_snapshot_path(
            "lustrefs_exporter__tests__lnetctl_stats.histsnap",
        ));

        compare_metrics(&current, &previous);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_lnetctl_stats_mds_otel() {
        let output = include_str!("../fixtures/stats_mds.json");

        let stats = encode_lustre_stats_from_fixture(output);

        insta::assert_snapshot!(stats);

        let current = get_scrape(stats);

        let previous = read_metrics_from_snapshot(&historical_snapshot_path(
            "lustrefs_exporter__tests__lnetctl_stats_mds.histsnap",
        ));

        compare_metrics(&current, &previous);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_host_stats_non_healthy_otel() {
        let output = include_str!("../fixtures/host_stats_non_healthy.json");

        let stats = encode_lustre_stats_from_fixture(output);

        insta::assert_snapshot!(stats);

        let current = get_scrape(stats);

        let previous = read_metrics_from_snapshot(&historical_snapshot_path(
            "lustrefs_exporter__tests__host_stats_non_healthy.histsnap",
        ));

        compare_metrics(&current, &previous);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn test_client_stats_otel() {
        let output = include_str!("../fixtures/client.json");

        let stats = encode_lustre_stats_from_fixture(output);

        insta::assert_snapshot!(stats);

        let current = get_scrape(stats);

        let previous = read_metrics_from_snapshot(&historical_snapshot_path(
            "lustrefs_exporter__tests__client_stats.histsnap",
        ));

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

                let previous = read_metrics_from_snapshot(&historical_snap);

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

    pub fn get_scrape(x: String) -> Scrape {
        let x = x.lines().map(|x| Ok(x.to_owned()));

        Scrape::parse(x).unwrap()
    }

    pub(super) fn read_metrics_from_snapshot(path: &Path) -> Scrape {
        let x = insta::Snapshot::from_file(path).unwrap();

        let insta::internals::SnapshotContents::Text(x) = x.contents() else {
            panic!("Snapshot is not text");
        };

        get_scrape(x.to_string())
    }

    fn parse_lustre_metrics(contents: &str) -> String {
        let (records, _) = parse()
            .easy_parse(contents)
            .map_err(|err| err.map_position(|p| p.translate_position(contents)))
            .unwrap();

        build_lustre_stats(&records)
    }

    fn encode_lustre_stats_from_fixture(content: &str) -> String {
        let records = serde_json::from_str(content).unwrap();

        build_lustre_stats(&records)
    }

    fn build_lustre_stats(x: &Vec<Record>) -> String {
        let mut registry = Registry::default();
        let mut metrics = Metrics::default();

        openmetrics::build_lustre_stats(x, &mut metrics);

        metrics.register_metric(&mut registry);

        let mut stats = String::new();

        encode(&mut stats, &registry).unwrap();

        stats
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
                    Some((k.clone(), v.strip_suffix(".").unwrap_or(v).to_string()))
                } else {
                    None
                }
            })
            .collect();

        sorted_docs.sort_by(|a, b| a.0.cmp(&b.0)); // Sort by key

        sorted_docs
    }
}

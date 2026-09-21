// Copyright (c) 2024 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

pub mod brw_stats;
pub mod client;
pub mod client_target;
pub mod controller;
pub mod histogram;
pub mod host;
pub mod jobstats;
pub mod llite;
pub mod lnet;
pub mod metrics;
pub mod quota;
pub mod routes;
pub mod service;
pub mod stats;
pub mod stream;

use crate::routes::{
    jobstats_metrics_cmd, lnet_global_output, lnet_stats_output, lustre_metrics_output,
    net_show_output,
};
use axum::{
    http::{self, StatusCode},
    response::{IntoResponse, Response},
};
use lustre_collector::{ControllerVariant, LustreCollectorError, TargetVariant};
use prometheus_client::metrics::family::Family as PrometheusFamily;

pub type LabelContainer = Vec<(&'static str, String)>;
pub type Family<T> = PrometheusFamily<LabelContainer, T>;

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
    #[error("Failed to reset md_stats: {0} (exit code: {1:?})")]
    MdtStatsReset(String, Option<i32>),
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

pub(crate) trait LabelProm {
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

impl LabelProm for ControllerVariant {
    fn to_prom_label(&self) -> &'static str {
        match self {
            ControllerVariant::Osc => "osc",
            ControllerVariant::Mdc => "mdc",
        }
    }
}

/// Dumps Lustre filesystem statistics to stdout
///
/// This function executes several Lustre commands and prints their raw output:
/// - `lctl get_param` with all standard parameters from the parser
/// - `lctl get_param` for jobstats (OST and MDT job statistics)
/// - `lnetctl net show -v 4` for network configuration details
/// - `lnetctl stats show` for network statistics
///
/// # Returns
/// * `Ok(())` on successful execution of all commands
/// * `Err(Error)` if any command fails or output cannot be converted to UTF-8
///
/// # Example
/// ```rust
/// use lustrefs_exporter::dump_stats;
///
/// async fn test_dump_stats() {
///     dump_stats().await.unwrap();
/// }
/// ```
pub async fn dump_stats() -> Result<(), Error> {
    println!("# Dumping lctl get_param output");

    let mut lctl = lustre_metrics_output();

    let lctl = lctl.output().await?;

    println!("{}", std::str::from_utf8(&lctl.stdout)?);

    println!("# Dumping lctl get_param jobstats output");

    let mut lctl = jobstats_metrics_cmd();

    let lctl = lctl.output().await?;

    println!("{}", std::str::from_utf8(&lctl.stdout)?);

    println!("# Dumping lnetctl net show output");

    let mut lnetctl = net_show_output();

    let lnetctl = lnetctl.output().await?;

    println!("{}", std::str::from_utf8(&lnetctl.stdout)?);

    println!("# Dumping lnetctl stats show output");

    let mut lnetctl_stats_output = lnet_stats_output();

    let lnetctl_stats_output = lnetctl_stats_output.output().await?;

    println!("{}", std::str::from_utf8(&lnetctl_stats_output.stdout)?);

    println!("# Dumping lnetctl global show output");

    let mut lnetctl_global_output = lnet_global_output();

    let lnetctl_global_output = lnetctl_global_output.output().await?;

    println!("{}", std::str::from_utf8(&lnetctl_global_output.stdout)?);

    Ok(())
}

#[cfg(test)]
pub mod tests {
    use crate::{
        Error, LabelProm as _,
        client::{ClientLabels, ExtendedClientMetrics},
        dump_stats,
        histogram::HistogramEncoding,
        metrics::{self, Metrics},
    };
    use axum::{http::StatusCode, response::IntoResponse as _};
    use combine::EasyParser as _;
    use commandeer_test::commandeer;
    use lustre_collector::{ControllerVariant, Record, TargetVariant, parser::parse};
    use prometheus_client::{encoding::text::encode, registry::Registry};
    use prometheus_parse::{Sample, Scrape, Value};
    use serial_test::serial;
    use std::{
        collections::HashSet,
        path::{Path, PathBuf},
    };

    // These metrics are ignored for the comparison with the previous implementation
    // since they are new and not present in the previous implementation.
    const IGNORED_METRICS: &[&str] = &[
        "lustre_cache_hit_total",
        "lustre_cache_access_total",
        "lustre_cache_miss_total",
        "lustre_client_llite_read_bytes_total",
        "lustre_client_llite_write_bytes_total",
        "lustre_exporter_parse_errors",
        "lustre_exporter_command_errors",
        "lustre_get_page_total",
        "lustre_page_size_bytes",
        "lustre_health_healthy",
        "lustre_health_value",
        "lustre_many_credits_total",
        "lustre_osc_state",
        "lustre_osp_active",
        "lustre_osp_max_create_count",
        "lustre_stats_time_max",
        "lustre_stats_time_min",
        "lustre_stats_time_total",
        "recovery_status",
        "recovery_status_completed_clients",
        "recovery_status_duration_seconds",
        "recovery_status_time_remaining_seconds",
        "recovery_status_total_clients",
        "target_info",
        // GCP-226: new _start_time companions, not present in the historical OTel baseline
        "lustre_brw_stats_start_time",
        "lustre_stats_start_time",
        "lustre_client_stats_start_time",
        "lustre_ldlm_canceld_stats_start_time",
        "lustre_ldlm_cbd_stats_start_time",
    ];

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

    #[test]
    fn test_controller_variant_to_prom_label() {
        assert_eq!(ControllerVariant::Osc.to_prom_label(), "osc");
        assert_eq!(ControllerVariant::Mdc.to_prom_label(), "mdc");
    }

    #[commandeer(Replay, "lctl", "lnetctl")]
    #[tokio::test]
    #[serial]
    async fn test_dump_stats() {
        dump_stats().await.unwrap();
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

                let x = parse_lustre_metrics(&contents, ClientLabels::PerTarget);

                insta::assert_snapshot!(x);

                let current = get_scrape(x);

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

    /// There are various differences between the current snapshots and the otel snapshots.
    /// It is imperative that the metrics between both snapshots are the same. However,
    /// we cannot do a direct comparison of the text as there are several differences in the
    /// way the data is encoded:
    /// 1. Metric descriptions: The otel implementation did not have trailing periods, while
    ///    the prometheus-client crate adds a period to the end of all metric descriptions.
    /// 2. Label ordering: Labels are not sorted alphabetically in the otel implementation,
    ///    while prometheus-client sorts them.
    /// 3. EOF marker: The otel version did not contain the `# EOF` line that is present
    ///    in the current implementation.
    /// 4. Removed metrics: The `target_info` metric has been removed in the new implementation.
    /// 5. Removed labels: The `otel_scope_name` label has been removed from all metrics.
    ///
    /// This test ensures that the current snapshots still match the otel snapshots by normalizing
    /// each line in both snapshot files before performing a comparison.
    #[test]
    fn compare_snapshots_to_existing_otel_snapshots() -> Result<(), Box<dyn std::error::Error>> {
        insta::glob!("otel_snapshots/", "*.otelsnap", |path| {
            let snap_name = path.file_name().unwrap();
            let snap_file = path
                .parent()
                .unwrap()
                .parent()
                .unwrap()
                .join("snapshots")
                .join(snap_name.to_string_lossy().replace(".otelsnap", ".snap"));
            let otel_metrics = read_metrics_from_snapshot(path);
            let metrics = read_metrics_from_snapshot(&snap_file);

            compare_metrics(&otel_metrics, &metrics);
        });

        Ok(())
    }

    pub(super) fn compare_metrics(metrics1: &Scrape, metrics2: &Scrape) {
        // Skip OTEL specific metric and updated metrics.
        let set1: HashSet<_> = metrics1
            .samples
            .iter()
            .filter(|s| !IGNORED_METRICS.contains(&s.metric.as_str()))
            .map(normalize_sample)
            .collect();

        let set2: HashSet<_> = metrics2
            .samples
            .iter()
            .filter(|s| !IGNORED_METRICS.contains(&s.metric.as_str()))
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
        // According to the Prometheus text exposition format specification,
        // curly braces {} are required even for empty label sets.
        // See: https://prometheus.io/docs/instrumenting/exposition_formats/#text-format-details
        // The format is: metric_name [ "{" label_name "=" `"` label_value `"` ... "}" ] value [ timestamp ]
        // The square brackets indicate the label section is optional, but when present,
        // the curly braces are part of the required syntax, even if no labels exist.
        // Therefore, as an example, "lustre_mem_used_max{} 1611219801" is the correct format,
        // not "lustre_mem_used_max 1611219801". However, `Scrape::parse` will not parse this correctly... So
        // it needs to be removed before parsing. This only affects testing.
        let x = x.replace("{}", "");

        let x = x.lines().map(|x| Ok(x.to_owned()));

        Scrape::parse(x).unwrap()
    }

    pub(super) fn read_metrics_from_snapshot(path: &Path) -> Scrape {
        let x = insta::Snapshot::from_file(path).unwrap_or_else(|e| {
            panic!("Could not read snapshot from {}: {e}", path.display());
        });

        let insta::internals::SnapshotContents::Text(x) = x.contents() else {
            panic!("Snapshot is not text");
        };

        get_scrape(x.to_string())
    }

    fn parse_lustre_metrics(contents: &str, client_labels: ClientLabels) -> String {
        let (records, _) = parse()
            .easy_parse(contents)
            .map_err(|err| err.map_position(|p| p.translate_position(contents)))
            .unwrap();

        build_lustre_stats(&records, client_labels, HistogramEncoding::BucketCounters)
    }

    /// The value of the one sample line for `series` (name and labels).
    fn series(output: &str, series: &str) -> String {
        let mut values = output
            .lines()
            .filter_map(|l| l.strip_prefix(series)?.strip_prefix(' '));
        let value = values
            .next()
            .unwrap_or_else(|| panic!("no series {series}"));

        assert!(values.next().is_none(), "more than one series {series}");

        value.to_string()
    }

    fn encode_lustre_stats_from_fixture(content: &str) -> String {
        let records = serde_json::from_str(content).unwrap();

        build_lustre_stats(
            &records,
            ClientLabels::PerTarget,
            HistogramEncoding::BucketCounters,
        )
    }

    #[test]
    fn osp_stats_under_osc_name_are_not_client_families() {
        let contents = "osc.lustre-OST0000-osc-MDT0000.stats=\nsnapshot_time             1789952232.668377739 secs.nsecs\nstart_time                1787073662.340515449 secs.nsecs\nelapsed_time              2878570.327862290 secs.nsecs\nreq_waittime              1383 samples [usecs] 12 3057 1421392 4185129512\nreq_active                1383 samples [reqs] 1 2 1414 1476\nost_connect               1 samples [usecs] 1205 1205 1205 1452025\nobd_ping                  1382 samples [usecs] 12 3057 1420187 4183677487\nosc.lustre-OST0000-osc-MDT0000.state=\ncurrent_state: FULL\nstate_history:\n";

        let x = parse_lustre_metrics(contents, ClientLabels::PerTarget);

        assert!(!x.contains("lustre_client_osc"), "{x}");
        assert!(x.contains("lustre_osc_state{controller=\"lustre-OST0000-osc-MDT0000\",current_state=\"FULL\"} 1"), "{x}");
    }

    #[test]
    fn lockless_truncates_are_exported() {
        let contents = "osc.lustre-OST0000-osc-ffff949bc0626000.osc_stats=\nsnapshot_time:            1689697369.331040915 secs.nsecs\nlockless_write_bytes\t\t0\nlockless_read_bytes\t\t8192\nlockless_truncate\t\t3\nmemused=1\n";

        let x = parse_lustre_metrics(contents, ClientLabels::PerTarget);

        assert!(x.contains("lustre_client_osc_lockless_truncates_total{fs=\"lustre\",target=\"lustre-OST0000-osc-ffff949bc0626000\"} 3\n"), "{x}");
        assert!(x.contains("lustre_client_osc_lockless_read_bytes_total{fs=\"lustre\",target=\"lustre-OST0000-osc-ffff949bc0626000\"} 8192\n"), "{x}");
    }

    #[test]
    fn client_fixture_aggregated_by_filesystem() {
        let contents = include_str!(
            "../../lustre-collector/src/fixtures/valid/lustre-2.14.0_ddn259/client/client.txt"
        );

        let x = parse_lustre_metrics(contents, ClientLabels::ByFilesystem);

        assert!(!x.contains("a361000-OST"), "{x}");
        assert!(!x.contains("a361000-MDT"), "{x}");
        assert_eq!(
            series(&x, "lustre_client_osc_cur_grant_bytes{fs=\"a361000\"}"),
            "33751040"
        );
        assert_eq!(
            series(
                &x,
                "lustre_client_osc_stats_total{fs=\"a361000\",operation=\"ldlm_cancel\"}"
            ),
            "32"
        );
        assert_eq!(
            series(
                &x,
                "lustre_client_osc_stats_time_microseconds_min{fs=\"a361000\",operation=\"ldlm_cancel\"}"
            ),
            "754"
        );
        assert_eq!(
            series(
                &x,
                "lustre_client_osc_stats_time_microseconds_max{fs=\"a361000\",operation=\"ldlm_cancel\"}"
            ),
            "1827"
        );
        assert_eq!(
            series(
                &x,
                "lustre_client_osc_stats_time_microseconds_total{fs=\"a361000\",operation=\"ldlm_cancel\"}"
            ),
            "37818"
        );
        assert_eq!(
            series(&x, "lustre_client_osc_stats_start_time{fs=\"a361000\"}"),
            "1790276704"
        );
        assert_eq!(
            series(
                &x,
                "lustre_osc_state{fs=\"a361000\",current_state=\"FULL\"}"
            ),
            "4"
        );
        assert!(
            x.contains("lustre_client_llite_read_bytes_total{fs=\"a361000\",target=\"a361000-"),
            "{x}"
        );

        insta::assert_snapshot!(x);
    }

    #[test]
    fn client_fixture_2_16_aggregated_by_filesystem() {
        let contents = include_str!(
            "../../lustre-collector/src/fixtures/valid/lustre-2.16.0_ddn56b/client/client.txt"
        );

        let x = parse_lustre_metrics(contents, ClientLabels::ByFilesystem);

        assert!(!x.contains("-OST"), "{x}");
        assert!(!x.contains("-MDT"), "{x}");
        assert_eq!(
            series(&x, "lustre_client_osc_cur_grant_bytes{fs=\"a361000\"}"),
            "29532160"
        );
        assert_eq!(
            series(&x, "lustre_client_osc_read_bytes_total{fs=\"a361000\"}"),
            "66647912448"
        );
        assert_eq!(
            series(
                &x,
                "lustre_client_osc_stats_total{fs=\"a361000\",operation=\"ldlm_cancel\"}"
            ),
            "2023004"
        );
        assert_eq!(
            series(
                &x,
                "lustre_client_osc_stats_time_microseconds_min{fs=\"a361000\",operation=\"ldlm_cancel\"}"
            ),
            "99"
        );
        assert_eq!(
            series(
                &x,
                "lustre_client_osc_stats_time_microseconds_max{fs=\"a361000\",operation=\"ldlm_cancel\"}"
            ),
            "31061"
        );
        assert_eq!(
            series(
                &x,
                "lustre_osc_state{fs=\"a361000\",current_state=\"FULL\"}"
            ),
            "4"
        );

        insta::assert_snapshot!(x);
    }

    fn parse_lustre_histograms(contents: &str, client_labels: ClientLabels) -> String {
        let (records, _) = parse()
            .easy_parse(contents)
            .map_err(|err| err.map_position(|p| p.translate_position(contents)))
            .unwrap();

        let x = build_lustre_stats(&records, client_labels, HistogramEncoding::Histogram);

        assert_valid_histograms(&x);

        x
    }

    /// What Prometheus checks before accepting a histogram: buckets ascend,
    /// are cumulative, and end in `+Inf` equal to `_count`.
    fn assert_valid_histograms(output: &str) {
        let scrape = get_scrape(output.to_string());
        let mut histograms = 0;

        for sample in &scrape.samples {
            let Value::Histogram(buckets) = &sample.value else {
                continue;
            };
            let name = format!("{}{:?}", sample.metric, sample.labels);

            for pair in buckets.windows(2) {
                assert!(pair[0].less_than < pair[1].less_than, "{name}");
                assert!(pair[0].count <= pair[1].count, "{name}");
            }

            let last = buckets.last().unwrap();
            assert_eq!(last.less_than, f64::INFINITY, "{name}");

            let count = scrape
                .samples
                .iter()
                .find(|s| {
                    s.metric == format!("{}_count", sample.metric) && s.labels == sample.labels
                })
                .unwrap_or_else(|| panic!("{name}: no _count"));
            assert_eq!(count.value, Value::Untyped(last.count), "{name}");

            histograms += 1;
        }

        assert!(histograms > 0);
    }

    #[test]
    fn client_fixture_as_histograms() {
        let contents = include_str!(
            "../../lustre-collector/src/fixtures/valid/lustre-2.14.0_ddn259/client/client.txt"
        );

        let x = parse_lustre_histograms(contents, ClientLabels::PerTarget);

        assert_eq!(
            series(
                &x,
                "lustre_client_llite_extents_stats_bucket{le=\"8191.0\",fs=\"a361000\",target=\"a361000-ffff8b524be17800\",operation=\"read\"}"
            ),
            "1"
        );
        insta::assert_snapshot!(x);

        let x = parse_lustre_histograms(contents, ClientLabels::ByFilesystem);

        insta::assert_snapshot!("client_fixture_as_histograms_aggregated", x);
    }

    #[test]
    fn rpc_stats_histogram_bounds() {
        let contents = "osc.a361000-OST0001-osc-ffff949bc0626000.rpc_stats=\nsnapshot_time:            1789952232.703630742 secs.nsecs\nstart_time:               1787864516.502993110 secs.nsecs\nelapsed_time:             2087716.200637632 secs.nsecs\nread RPCs in flight:  0\nwrite RPCs in flight: 0\nDIO RPCs in flight: 0\npending write pages:  0\npending read pages:   0\n\n\t\t\tread\t\t\twrite\npages per rpc         rpcs   % cum % |       rpcs   % cum %\n1:\t\t         0   0   0   |    52489  50  50\n2:\t\t         0   0   0   |    36540  35  85\n4:\t\t         1   0   0   |    14030  13  99\n8:\t\t       511  99 100   |      784   0 100\n\n\t\t\tread\t\t\twrite\nrpcs in flight        rpcs   % cum % |       rpcs   % cum %\n1:\t\t       512 100 100   |   103843 100 100\n\n\t\t\tread\t\t\twrite\noffset                rpcs   % cum % |       rpcs   % cum %\n0:\t\t       512 100 100   |   103843 100 100\n\n\t\t\tread\t\t\twrite\nRPC latency (us)       count   % cum % |       count   % cum %\n0:\t\t         0   0   0   |        0   0   0\n512:\t\t         3   0   0   |      567   0   0\n1024:\t\t       327  63  64   |    93264  89  90\n2048:\t\t       159  31  95   |     9820   9  99\n4096:\t\t        19   3  99   |      180   0  99\n8192:\t\t         3   0 100   |       10   0  99\n16384:\t\t         1   0 100   |        2   0 100\n";

        let x = parse_lustre_histograms(contents, ClientLabels::ByFilesystem);

        let bucket = |name: &str, op: &str, le: &str| {
            series(
                &x,
                &format!("{name}_bucket{{le=\"{le}\",fs=\"a361000\",operation=\"{op}\"}}"),
            )
        };
        let lat = "lustre_client_osc_rpc_stats_latency_microseconds";
        assert_eq!(bucket(lat, "read", "1.0"), "0");
        assert_eq!(bucket(lat, "read", "1024.0"), "3");
        assert_eq!(bucket(lat, "read", "2048.0"), "330");
        assert_eq!(bucket(lat, "read", "4096.0"), "489");
        assert_eq!(bucket(lat, "read", "32768.0"), "512");
        assert_eq!(bucket(lat, "read", "+Inf"), "512");
        assert_eq!(bucket(lat, "write", "2048.0"), "93831");
        assert!(!x.contains(&format!("{lat}_bucket{{le=\"512.0\"")));

        let pages = "lustre_client_osc_rpc_stats_pages_per_rpc";
        assert_eq!(bucket(pages, "read", "4.0"), "1");
        assert_eq!(bucket(pages, "read", "8.0"), "512");
        assert_eq!(bucket(pages, "write", "1.0"), "52489");

        assert_eq!(
            bucket("lustre_client_osc_rpc_stats_offset", "read", "0.0"),
            "512"
        );
        assert_eq!(
            bucket("lustre_client_osc_rpc_stats_rpcs_in_flight", "write", "1.0"),
            "103843"
        );
    }

    #[test]
    fn client_fixture_extended() {
        let contents = include_str!(
            "../../lustre-collector/src/fixtures/valid/lustre-2.14.0_ddn259/client/client.txt"
        );

        let (records, _) = parse()
            .easy_parse(contents)
            .map_err(|err| err.map_position(|p| p.translate_position(contents)))
            .unwrap();

        let x = build_lustre_stats_extended(
            &records,
            ClientLabels::PerTarget,
            HistogramEncoding::BucketCounters,
            ExtendedClientMetrics::On { page_size: 65536 },
        );

        let scrape = get_scrape(x.clone());
        let sizes = |family: &str| -> Vec<(u64, u64)> {
            let mut xs: Vec<(u64, u64)> = scrape
                .samples
                .iter()
                .filter(|s| s.metric == format!("{family}_total"))
                .map(|s| {
                    let Value::Counter(rpcs) = &s.value else {
                        panic!("{}", s.metric)
                    };

                    (s.labels["size"].parse().unwrap(), *rpcs as u64)
                })
                .collect();
            xs.sort_unstable();
            xs
        };

        for component in ["osc", "mdc"] {
            let pages = sizes(&format!(
                "lustre_client_{component}_rpc_stats_pages_per_rpc"
            ));
            let bytes = sizes(&format!(
                "lustre_client_{component}_rpc_stats_bytes_per_rpc"
            ));

            assert!(!pages.is_empty(), "{component}");
            assert_eq!(
                bytes,
                pages
                    .iter()
                    .map(|&(pages, rpcs)| (pages * 65536, rpcs))
                    .collect::<Vec<_>>(),
                "{component}"
            );
        }

        let without_bytes: String = x
            .lines()
            .filter(|l| !l.contains("_rpc_stats_bytes_per_rpc"))
            .map(|l| format!("{l}\n"))
            .collect();
        assert_eq!(
            without_bytes,
            build_lustre_stats(
                &records,
                ClientLabels::PerTarget,
                HistogramEncoding::BucketCounters
            )
        );

        let x = build_lustre_stats_extended(
            &records,
            ClientLabels::ByFilesystem,
            HistogramEncoding::Histogram,
            ExtendedClientMetrics::On { page_size: 4096 },
        );

        assert_valid_histograms(&x);

        let sum = |name: &str| -> f64 {
            series(
                &x,
                &format!("{name}_sum{{fs=\"a361000\",operation=\"write\"}}"),
            )
            .parse()
            .unwrap()
        };
        assert_eq!(
            sum("lustre_client_osc_rpc_stats_bytes_per_rpc"),
            sum("lustre_client_osc_rpc_stats_pages_per_rpc") * 4096.0
        );
        insta::assert_snapshot!("client_fixture_extended_aggregated_histograms", x);
    }

    fn build_lustre_stats(
        x: &Vec<Record>,
        client_labels: ClientLabels,
        histograms: HistogramEncoding,
    ) -> String {
        build_lustre_stats_extended(x, client_labels, histograms, ExtendedClientMetrics::Off)
    }

    fn build_lustre_stats_extended(
        x: &Vec<Record>,
        client_labels: ClientLabels,
        histograms: HistogramEncoding,
        extended: ExtendedClientMetrics,
    ) -> String {
        let mut registry = Registry::default();
        let mut metrics = Metrics::new(client_labels, histograms, extended);

        metrics::build_lustre_stats(x, &mut metrics);

        metrics.register_metric(&mut registry);

        let mut stats = String::new();

        encode(&mut stats, &registry).unwrap();

        stats
    }

    fn normalize_sample(sample: &Sample) -> (String, Vec<(String, String)>, String) {
        let mut sorted_labels: Vec<_> = sample
            .labels
            .iter()
            .filter(|(k, _)| *k != "otel_scope_name")
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
                if !IGNORED_METRICS.contains(&k.as_str()) {
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

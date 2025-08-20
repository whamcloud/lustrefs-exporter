// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

mod jobstats;
use axum::{
    Router,
    body::{Body, to_bytes},
    http::Request,
};
use combine::parser::EasyParser;
use include_dir::{Dir, include_dir};
use insta::assert_snapshot;
use lustre_collector::parser::parse;
use lustrefs_exporter::openmetrics::{self, Metrics};
use lustrefs_exporter::{JobstatsMock, LustreMock, create_mock_commander, routes};
use prometheus_client::{encoding::text::encode, registry::Registry};
use prometheus_parse::{Sample, Scrape};
use sealed_test::prelude::*;
use std::{
    collections::{HashMap, HashSet},
    env,
    error::Error,
    fs,
};
use tower::util::ServiceExt;

static VALID_FIXTURES: Dir<'_> =
    include_dir!("$CARGO_MANIFEST_DIR/../lustre-collector/src/fixtures/valid/");

async fn test_valid_fixtures_otel() {
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

                let mut registry = Registry::default();
                let mut metrics = Metrics::default();

                openmetrics::build_lustre_stats(&result.0, &mut metrics);

                metrics.register_metric(&mut registry);

                let mut buffer = String::new();
                encode(&mut buffer, &registry).unwrap();

                assert_snapshot!(format!("valid_fixture_otel_{name}"), buffer);
            }
        }
    }
}

async fn build_lustre_stats(output: &str) -> Registry {
    let x = serde_json::from_str(output).unwrap();

    let mut registry = Registry::default();
    let mut metrics = Metrics::default();

    openmetrics::build_lustre_stats(&x, &mut metrics);

    metrics.register_metric(&mut registry);

    registry
}

fn scrape_metrics(name: &str, otel_name: &str) -> (Scrape, Scrape) {
    let opentelemetry = read_metrics_from_snapshot(
        format!(
            "{}/tests/snapshots/{otel_name}.snap",
            env!("CARGO_MANIFEST_DIR")
        )
        .as_str(),
    )
    .unwrap();

    let previous_implementation = read_metrics_from_snapshot(
        format!("{}/tests/snapshots/{name}.snap", env!("CARGO_MANIFEST_DIR")).as_str(),
    )
    .unwrap();

    (opentelemetry, previous_implementation)
}

#[tokio::test(flavor = "multi_thread")]
async fn test_stats_otel() {
    let output = include_str!("../fixtures/stats.json");

    let registry = build_lustre_stats(output).await;

    let mut buffer = String::new();
    encode(&mut buffer, &registry).unwrap();

    insta::assert_snapshot!(buffer);

    let (opentelemetry, previous_implementation) =
        scrape_metrics("r#mod__stats", "r#mod__stats_otel");

    compare_metrics(&opentelemetry, &previous_implementation);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_lnetctl_stats_otel() {
    let output = include_str!("../fixtures/lnetctl_stats.json");

    let registry = build_lustre_stats(output).await;

    let mut buffer = String::new();
    encode(&mut buffer, &registry).unwrap();

    insta::assert_snapshot!(buffer);

    let (opentelemetry, previous_implementation) =
        scrape_metrics("r#mod__lnetctl_stats", "r#mod__lnetctl_stats_otel");

    compare_metrics(&opentelemetry, &previous_implementation);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_lnetctl_stats_mds_otel() {
    let output = include_str!("../fixtures/stats_mds.json");

    let registry = build_lustre_stats(output).await;

    let mut buffer = String::new();
    encode(&mut buffer, &registry).unwrap();

    insta::assert_snapshot!(buffer);

    let (opentelemetry, previous_implementation) =
        scrape_metrics("r#mod__lnetctl_stats_mds", "r#mod__lnetctl_stats_mds_otel");

    compare_metrics(&opentelemetry, &previous_implementation);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_host_stats_non_healthy_otel() {
    let output = include_str!("../fixtures/host_stats_non_healthy.json");

    let registry = build_lustre_stats(output).await;

    let mut buffer = String::new();
    encode(&mut buffer, &registry).unwrap();

    insta::assert_snapshot!(buffer);

    let (opentelemetry, previous_implementation) = scrape_metrics(
        "r#mod__host_stats_non_healthy",
        "r#mod__host_stats_non_healthy_otel",
    );

    compare_metrics(&opentelemetry, &previous_implementation);
}

#[tokio::test(flavor = "multi_thread")]
async fn test_client_stats_otel() {
    let output = include_str!("../fixtures/client.json");

    let registry = build_lustre_stats(output).await;

    let mut buffer = String::new();
    encode(&mut buffer, &registry).unwrap();

    insta::assert_snapshot!(buffer);

    let (opentelemetry, previous_implementation) =
        scrape_metrics("r#mod__client_stats", "r#mod__client_stats_otel");

    compare_metrics(&opentelemetry, &previous_implementation);
}
use pretty_assertions::assert_eq;

// Make sure metrics from the OpenTelemetry implementation are the same as the previous implementation
#[tokio::test(flavor = "multi_thread")]
async fn test_legacy_metrics() -> Result<(), Box<dyn std::error::Error>> {
    // Generate snapshots for the OpenTelemetry implementation
    test_valid_fixtures_otel().await;

    // Compare snapshots
    for dir in VALID_FIXTURES.find("*").unwrap() {
        match dir {
            include_dir::DirEntry::Dir(_) => {}
            include_dir::DirEntry::File(file) => {
                let name = file.path().to_string_lossy().to_string().replace("/", "__");
                // Useful when debugging
                // println!(
                //     "{}",
                //     format!(
                //         "{}/tests/snapshots/r#mod__valid_fixture_otel_{name}.snap",
                //         env!("CARGO_MANIFEST_DIR")
                //     )
                // );

                let (opentelemetry, previous_implementation) = scrape_metrics(
                    &format!("r#mod__valid_fixture_{name}"),
                    &format!("r#mod__valid_fixture_otel_{name}"),
                );

                compare_metrics(&opentelemetry, &previous_implementation);
            }
        }
    }
    Ok(())
}

fn get_app() -> (Request<Body>, Router) {
    let app = routes::app();

    let request = Request::builder()
        .uri("/metrics?jobstats=true")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    (request, app)
}

#[sealed_test]
fn test_metrics_endpoint() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let mock_commander = create_mock_commander(JobstatsMock::default(), LustreMock::default());

        let (request, app) = get_app();

        let resp = app.oneshot(request).await.unwrap();

        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let original_body_str = std::str::from_utf8(&body).unwrap();

        drop(mock_commander);

        let _mock_commander = create_mock_commander(JobstatsMock::default(), LustreMock::default());

        let (request, app) = get_app();

        let resp = app.oneshot(request).await.unwrap();

        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let body_str = std::str::from_utf8(&body).unwrap();

        assert_eq!(
            original_body_str, body_str,
            "Stats not the same after second scrape"
        );

        insta::assert_snapshot!("metrics_endpoint", original_body_str);
    });
}

#[sealed_test]
fn test_metrics_endpoint_multiple_calls_different_data() {
    tokio::runtime::Runtime::new().unwrap().block_on(async {
        let mock_commander =
            create_mock_commander(JobstatsMock::Lustre2_14_0_162, LustreMock::default());

        let (request, app) = get_app();

        let resp = app.oneshot(request).await.unwrap();

        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let original_body_str = std::str::from_utf8(&body).unwrap();

        insta::assert_snapshot!(
            "metrics_endpoint_multiple_calls_different_data",
            original_body_str
        );

        drop(mock_commander);

        let _mock_commander = create_mock_commander(JobstatsMock::default(), LustreMock::ValidMds);

        let (request, app) = get_app();

        let resp = app.oneshot(request).await.unwrap();

        let body = to_bytes(resp.into_body(), usize::MAX).await.unwrap();
        let body_str = std::str::from_utf8(&body).unwrap();

        insta::assert_snapshot!("metrics_endpoint_multiple_calls_different_data-2", body_str);
    });
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

    let metrics1_keys = metrics1.docs.keys().collect::<HashSet<_>>();
    let metrics2_keys = metrics2.docs.keys().collect::<HashSet<_>>();

    // The prometheus-client crate creates `# Help` and `# Type` entries for each metric,
    // even if the metric is not used. This means the encoded output will contain
    // additional metadata that is valid, but is **not** in non-otel metrics output.
    // Filter this metadata out to perform an accurate doc comparison.
    let doc_keys = metrics1_keys
        .intersection(&metrics2_keys)
        .collect::<Vec<_>>();

    let metrics1_docs = metrics1
        .docs
        .clone()
        .into_iter()
        .filter(|(k, _)| doc_keys.contains(&&k))
        .collect::<HashMap<_, _>>();

    let metrics2_docs = metrics2
        .docs
        .clone()
        .into_iter()
        .filter(|(k, _)| doc_keys.contains(&&k))
        .collect::<HashMap<_, _>>();

    // Normalize and compare metrics help
    let normalized_docs1 = normalize_docs(&metrics1_docs);
    let normalized_docs2 = normalize_docs(&metrics2_docs);

    assert_eq!(
        normalized_docs1, normalized_docs2,
        "Metrics help are not the same"
    );
}

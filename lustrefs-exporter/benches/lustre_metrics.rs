// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use iai_callgrind::{
    Callgrind, CallgrindMetrics, FlamegraphConfig, FlamegraphKind, LibraryBenchmarkConfig,
    OutputFormat, library_benchmark, library_benchmark_group, main,
};
use lustre_collector::{Record, parse_lnetctl_output, parse_lnetctl_stats};
use lustrefs_exporter::openmetrics::{self, OpenTelemetryMetrics};
use opentelemetry::metrics::MeterProvider;
use prometheus::{Encoder as _, TextEncoder};
use std::hint::black_box;

fn generate_records() -> Vec<Record> {
    let mut records = Vec::new();

    let lustre_metrics = include_str!(
        "../../lustre-collector/src/fixtures/valid/lustre-2.14.0_ddn133/2.14.0_ddn133_quota.txt"
    );
    let mut lustre_metrics_records =
        lustre_collector::parse_lctl_output(lustre_metrics.as_bytes()).unwrap();
    records.append(&mut lustre_metrics_records);

    let net_show = include_str!("../fixtures/lnetctl_net_show.txt");
    let mut net_show_records = parse_lnetctl_output(net_show).unwrap();
    records.append(&mut net_show_records);

    let net_stats = include_str!("../fixtures/lnetctl_stats.txt");
    let mut net_stats_records = parse_lnetctl_stats(&net_stats).unwrap();
    records.append(&mut net_stats_records);

    records
}

fn encode_metrics(records: Vec<Record>) {
    let (provider, registry) = lustrefs_exporter::init_opentelemetry().unwrap();

    let meter = provider.meter("test");

    let opentelemetry_metrics = OpenTelemetryMetrics::new(meter.clone());

    let mut lustre_stats = vec![];
    let encoder = TextEncoder::new();
    let metric_families = registry.gather();
    let _encoder_results = encoder.encode(&metric_families, &mut lustre_stats);

    // Build OTEL metrics
    openmetrics::build_lustre_stats(&records, opentelemetry_metrics);

    let mut buffer = vec![];
    let encoder = TextEncoder::new();
    let metric_families = registry.gather();
    encoder.encode(&metric_families, &mut buffer).unwrap();

    let lustre_stats = String::from_utf8_lossy(&buffer).to_string();

    println!("{}", lustre_stats);
}

#[library_benchmark]
#[benches::with_setup(setup = generate_records)]
fn bench_encode_lustre_metrics(records: Vec<Record>) {
    black_box(encode_metrics(records))
}

library_benchmark_group!(name = memory_benches; benchmarks = bench_encode_lustre_metrics);
main!(
    config = LibraryBenchmarkConfig::default()
        .tool(Callgrind::default()
            .format([CallgrindMetrics::All])
            .flamegraph(FlamegraphConfig::default().kind(FlamegraphKind::Differential)))
        .output_format(OutputFormat::default()
            .truncate_description(None)
        );
    library_benchmark_groups = memory_benches
);

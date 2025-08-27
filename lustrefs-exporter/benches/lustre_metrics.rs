// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use iai_callgrind::{
    Callgrind, CallgrindMetrics, FlamegraphConfig, FlamegraphKind, LibraryBenchmarkConfig,
    OutputFormat, library_benchmark, library_benchmark_group, main,
};
use lustre_collector::{Record, parse_lnetctl_output, parse_lnetctl_stats};
use lustrefs_exporter::metrics::{Metrics, build_lustre_stats};
use prometheus_client::{encoding::text::encode, registry::Registry};
use std::hint::black_box;

fn generate_records() -> Vec<Record> {
    let mut records = Vec::new();

    let lustre_metrics = include_str!(
        "../../lustre-collector/src/fixtures/valid/lustre-2.14.0_ddn133/2.14.0_ddn133_quota.txt"
    );
    let mut lustre_metrics_records = lustre_collector::parse_lctl_output(lustre_metrics.as_bytes())
        .expect("Failed to parse lustre metrics");
    records.append(&mut lustre_metrics_records);

    let net_show = include_bytes!("../fixtures/lnetctl_net_show.txt");
    let mut net_show_records =
        parse_lnetctl_output(net_show).expect("Failed to parse lnetctl net show");
    records.append(&mut net_show_records);

    let net_stats = include_bytes!("../fixtures/lnetctl_stats.txt");
    let mut net_stats_records =
        parse_lnetctl_stats(net_stats).expect("Failed to parse lnetctl stats");
    records.append(&mut net_stats_records);

    records
}

fn encode_metrics(records: Vec<Record>) -> Vec<u8> {
    let mut registry = Registry::default();
    let mut metrics = Metrics::default();

    // Build metrics
    build_lustre_stats(&records, &mut metrics);

    metrics.register_metric(&mut registry);

    let mut output = String::new();

    encode(&mut output, &registry).expect("Failed to encode metrics");

    output.as_bytes().to_vec()
}

#[library_benchmark]
#[benches::with_setup(setup = generate_records)]
fn bench_encode_lustre_metrics(records: Vec<Record>) -> Vec<u8> {
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

// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

mod common;

use common::load_test_concurrent;
use iai_callgrind::{
    Callgrind, CallgrindMetrics, FlamegraphConfig, FlamegraphKind, LibraryBenchmarkConfig,
    OutputFormat, library_benchmark, library_benchmark_group, main,
};

use crate::common::setup_env;

#[library_benchmark]
fn bench_memory_usage() {
    let rt = tokio::runtime::Runtime::new().unwrap();

    setup_env();

    rt.block_on(async { load_test_concurrent(10, 100).await });
}

library_benchmark_group!(name = memory_benches; benchmarks = bench_memory_usage);
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

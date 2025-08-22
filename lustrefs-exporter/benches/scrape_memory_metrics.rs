// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

mod common;

use commandeer_test::commandeer;
use common::load_test_concurrent;
use core::f64;
use criterion::{Criterion, criterion_group, criterion_main};
use std::time::Duration;
use sysinfo::{Pid, ProcessExt, System, SystemExt};

#[derive(serde::Serialize)]
struct BencherOutput {
    scrape_allocations: BencherMetrics,
}

#[derive(serde::Serialize)]
struct BencherMetrics {
    start_rss_mib: MetricEntry,
    peak_rss_mib: MetricEntry,
    end_rss_mib: MetricEntry,
    memory_growth_mib: MetricEntry,
    peak_over_start_rss_ratio: MetricEntry,
    avg_runtime_rss_mib: MetricEntry,
    start_virtual_mib: MetricEntry,
    peak_virtual_mib: MetricEntry,
    end_virtual_mib: MetricEntry,
    virtual_growth_mib: MetricEntry,
    peak_over_start_virtual_ratio: MetricEntry,
    avg_runtime_virtual_mib: MetricEntry,
}

#[derive(serde::Serialize)]
struct MetricEntry {
    value: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    lower_value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    upper_value: Option<f64>,
}

#[derive(Clone, Debug, serde::Serialize)]
struct MemoryUsage {
    start_rss: f64,
    end_rss: f64,
    memory_growth: f64,
    peak_over_start_rss_ratio: f64,
    avg_rss: f64,
    min_rss: f64,
    max_rss: f64,
    start_virtual: f64,
    end_virtual: f64,
    virtual_growth: f64,
    peak_over_start_virtual_ratio: f64,
    avg_virtual: f64,
    min_virtual: f64,
    max_virtual: f64,
}

impl From<MemoryUsage> for BencherOutput {
    fn from(x: MemoryUsage) -> Self {
        BencherOutput {
            scrape_allocations: BencherMetrics {
                start_rss_mib: MetricEntry {
                    value: x.start_rss,
                    lower_value: None,
                    upper_value: None,
                },
                peak_rss_mib: MetricEntry {
                    value: x.max_rss,
                    lower_value: None,
                    upper_value: None,
                },
                end_rss_mib: MetricEntry {
                    value: x.end_rss,
                    lower_value: None,
                    upper_value: None,
                },
                memory_growth_mib: MetricEntry {
                    value: x.memory_growth,
                    lower_value: None,
                    upper_value: None,
                },
                peak_over_start_rss_ratio: MetricEntry {
                    value: x.peak_over_start_rss_ratio,
                    lower_value: None,
                    upper_value: None,
                },
                avg_runtime_rss_mib: MetricEntry {
                    value: x.avg_rss,
                    lower_value: Some(x.min_rss),
                    upper_value: Some(x.max_rss),
                },
                start_virtual_mib: MetricEntry {
                    value: x.start_virtual,
                    lower_value: None,
                    upper_value: None,
                },
                peak_virtual_mib: MetricEntry {
                    value: x.max_virtual,
                    lower_value: None,
                    upper_value: None,
                },
                end_virtual_mib: MetricEntry {
                    value: x.end_virtual,
                    lower_value: None,
                    upper_value: None,
                },
                virtual_growth_mib: MetricEntry {
                    value: x.virtual_growth,
                    lower_value: None,
                    upper_value: None,
                },
                peak_over_start_virtual_ratio: MetricEntry {
                    value: x.peak_over_start_virtual_ratio,
                    lower_value: None,
                    upper_value: None,
                },
                avg_runtime_virtual_mib: MetricEntry {
                    value: x.avg_virtual,
                    lower_value: Some(x.min_virtual),
                    upper_value: Some(x.max_virtual),
                },
            },
        }
    }
}

fn get_memory_stats() -> (f64, f64) {
    let mut system = System::new();
    system.refresh_process(Pid::from(std::process::id() as usize));

    if let Some(process) = system.process(Pid::from(std::process::id() as usize)) {
        (process.memory() as f64, process.virtual_memory() as f64)
    } else {
        (0.0, 0.0)
    }
}

async fn load_test_with_memory_tracking(
    concurrency: usize,
    total_requests: usize,
) -> (Duration, MemoryUsage) {
    let (start_rss, start_virtual) = get_memory_stats();
    let mut rss_values = vec![];
    let mut virtual_values = vec![];

    rss_values.push(start_rss);
    virtual_values.push(start_virtual);

    let (tx, mut rx) = tokio::sync::oneshot::channel::<()>();
    let monitor_handle = tokio::spawn(async move {
        let mut min_rss = start_rss;
        let mut peak_rss = start_rss;
        let mut min_virtual = start_virtual;
        let mut peak_virtual = start_virtual;
        let mut interval = tokio::time::interval(Duration::from_millis(100));

        while rx.try_recv().is_err() {
            interval.tick().await;

            let (current_rss, current_virtual) = get_memory_stats();

            rss_values.push(current_rss);
            min_rss = min_rss.min(current_rss);
            peak_rss = peak_rss.max(current_rss);

            virtual_values.push(current_virtual);
            min_virtual = min_virtual.min(current_virtual);
            peak_virtual = peak_virtual.max(current_virtual);
        }

        (
            rss_values,
            min_rss,
            peak_rss,
            virtual_values,
            min_virtual,
            peak_virtual,
        )
    });

    let duration = load_test_concurrent(concurrency, total_requests).await;

    tx.send(())
        .expect("Failed to send stop signal to memory monitor");

    let (rss_values, mut min_rss, mut max_rss, virtual_values, mut min_virtual, mut max_virtual) =
        monitor_handle
            .await
            .expect("Failed to collect memory metrics from run.");

    let (end_rss, end_virtual) = get_memory_stats();

    min_rss = min_rss.min(end_rss);
    max_rss = max_rss.max(end_rss);

    min_virtual = min_virtual.min(end_virtual);
    max_virtual = max_virtual.max(end_virtual);

    let memory_usage = MemoryUsage {
        start_rss,
        end_rss,
        memory_growth: end_rss - start_rss,
        peak_over_start_rss_ratio: max_rss / start_rss,
        avg_rss: rss_values.iter().sum::<f64>() / rss_values.len() as f64,
        min_rss,
        max_rss,
        start_virtual,
        end_virtual,
        virtual_growth: end_virtual - start_virtual,
        peak_over_start_virtual_ratio: max_virtual / start_virtual,
        avg_virtual: virtual_values.iter().sum::<f64>() / virtual_values.len() as f64,
        min_virtual,
        max_virtual,
    };

    (duration, memory_usage)
}

#[commandeer(Replay, "lctl", "lnetctl")]
fn scrape_load_test(c: &mut Criterion) {
    let (tx, rx) = std::sync::mpsc::channel::<MemoryUsage>();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_time()
        .enable_io()
        .build()
        .expect("Failed to build tokio runtime");

    let mut group = c.benchmark_group("scrape_benchmarks");

    // Load test benchmark (like oha: 1000 requests, 10 concurrent)
    group.sample_size(15); // Fewer samples since each does 1000 requests
    group.measurement_time(Duration::from_secs(120)); // Allow more time

    group.bench_function("load_test_1000_req_10_concurrent_sequential", |b| {
        let tx = tx.clone();

        b.to_async(&rt).iter(|| async {
            let (duration, memory_usage) = load_test_with_memory_tracking(10, 1000).await;

            let _ = tx.send(memory_usage.clone());

            std::hint::black_box((duration, memory_usage))
        })
    });

    group.finish();

    drop(tx);

    let samples = rx.iter().collect::<Vec<_>>();

    if !samples.is_empty() {
        let aggregated = aggregate_samples(&samples);

        let bencher_output: BencherOutput = aggregated.into();

        let serialized_metrics = serde_json::to_string_pretty(&bencher_output)
            .expect("Failed to serialize benchmark output.");

        std::fs::write("scrape_allocations_results.json", serialized_metrics)
            .expect("Failed to write benchmark results to `scrape_allocations_results.json`");

        println!("✅ Bencher results written to scrape_allocations_results.json");
    }
}

fn aggregate_samples(samples: &[MemoryUsage]) -> MemoryUsage {
    let size = samples.len() as f64;
    const MIB_CONVERSION_FACTOR: f64 = 1_048_576.0;

    MemoryUsage {
        start_rss: samples.iter().map(|x| x.start_rss).sum::<f64>() / size / MIB_CONVERSION_FACTOR,
        end_rss: samples.iter().map(|x| x.end_rss).sum::<f64>() / size / MIB_CONVERSION_FACTOR,
        memory_growth: samples.iter().map(|x| x.memory_growth).sum::<f64>()
            / size
            / MIB_CONVERSION_FACTOR,
        peak_over_start_rss_ratio: samples
            .iter()
            .map(|x| x.peak_over_start_rss_ratio)
            .sum::<f64>()
            / size,
        avg_rss: samples.iter().map(|x| x.avg_rss).sum::<f64>() / size / MIB_CONVERSION_FACTOR,
        min_rss: samples
            .iter()
            .map(|x| x.min_rss)
            .fold(f64::INFINITY, f64::min)
            / MIB_CONVERSION_FACTOR,
        max_rss: samples
            .iter()
            .map(|x| x.max_rss)
            .fold(f64::NEG_INFINITY, f64::max)
            / MIB_CONVERSION_FACTOR,

        start_virtual: samples.iter().map(|x| x.start_virtual).sum::<f64>()
            / size
            / MIB_CONVERSION_FACTOR,
        end_virtual: samples.iter().map(|x| x.end_virtual).sum::<f64>()
            / size
            / MIB_CONVERSION_FACTOR,
        virtual_growth: samples.iter().map(|x| x.virtual_growth).sum::<f64>()
            / size
            / MIB_CONVERSION_FACTOR,
        peak_over_start_virtual_ratio: samples
            .iter()
            .map(|x| x.peak_over_start_virtual_ratio)
            .sum::<f64>()
            / size,
        avg_virtual: samples.iter().map(|x| x.avg_virtual).sum::<f64>()
            / size
            / MIB_CONVERSION_FACTOR,
        min_virtual: samples
            .iter()
            .map(|x| x.min_virtual)
            .fold(f64::INFINITY, f64::min)
            / MIB_CONVERSION_FACTOR,
        max_virtual: samples
            .iter()
            .map(|x| x.max_virtual)
            .fold(f64::NEG_INFINITY, f64::max)
            / MIB_CONVERSION_FACTOR,
    }
}

criterion_group!(benches, scrape_load_test);
criterion_main!(benches);

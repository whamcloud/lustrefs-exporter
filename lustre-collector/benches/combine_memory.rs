use combine::parser::EasyParser;
use criterion::{Criterion, criterion_group, criterion_main};
use lustre_collector::quota::parse as combine_parse;
use memory_benchmarking::{BencherOutput, MemoryUsage, aggregate_samples, get_memory_stats};
use std::{fs::File, io::Read, time::Duration};

async fn test_combine_with_mem(buffer: &str) -> MemoryUsage {
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

    let mut needle = buffer;
    while let Ok((_, e)) = combine_parse().easy_parse(needle) {
        needle = e;
    }

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

    MemoryUsage {
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
    }
}

pub fn combine_memory(c: &mut Criterion) {
    let (tx, rx) = std::sync::mpsc::channel::<MemoryUsage>();

    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_time()
        .enable_io()
        .build()
        .expect("Failed to build tokio runtime");

    let mut group = c.benchmark_group("parse_benchmarks");

    group.sample_size(10);
    group.measurement_time(Duration::from_secs(90)); // Allow more time

    let mut raw = String::new();
    File::open("benches/quotas.yml")
        .expect("Failed to open file")
        .read_to_string(&mut raw)
        .expect("Failed to read file");

    group.bench_with_input("combine_memory", &raw, |b, input| {
        b.to_async(&rt).iter(|| async {
            let memory_usage = test_combine_with_mem(input).await;

            let _ = tx.send(memory_usage.clone());
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

        let output = "combine_mem_usage.json";
        std::fs::write(output, serialized_metrics).expect("Failed to write benchmark results");

        println!("✅ Bencher results written to {output}");
    }
}

criterion_group!(benches, combine_memory);
criterion_main!(benches);

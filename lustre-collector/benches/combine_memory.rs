use combine::parser::EasyParser;
use criterion::{Criterion, criterion_group, criterion_main};
use lustre_collector::quota::parse as combine_parse;
use memory_benchmarking::{
    BencherOutput, Error as MemBenchError, MemoryUsage, aggregate_samples, sample_memory,
};
use std::{fs::File, io::Read, time::Duration};

async fn test_combine_with_mem(buffer: &str) -> Result<MemoryUsage, MemBenchError> {
    let mut samples = vec![sample_memory()];

    let (tx, mut rx) = tokio::sync::oneshot::channel::<()>();
    let monitor_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(100));

        while rx.try_recv().is_err() {
            interval.tick().await;

            samples.push(sample_memory());
        }

        samples
    });

    let mut needle = buffer;
    while let Ok((_, e)) = combine_parse().easy_parse(needle) {
        needle = e;
    }

    tx.send(())
        .expect("Failed to send stop signal to memory monitor");

    let mut samples = monitor_handle
        .await
        .expect("Failed to collect memory metrics from run.");

    samples.push(sample_memory());

    let mem = MemoryUsage::try_from(samples.as_slice())?;

    Ok(mem)
}

pub fn combine_memory(c: &mut Criterion) {
    let (tx, rx) = std::sync::mpsc::channel();

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

    let samples = rx
        .iter()
        .collect::<Result<Vec<_>, _>>()
        .expect("Failed to get memory information");

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

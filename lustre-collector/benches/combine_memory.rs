use combine::parser::EasyParser;
use criterion::{Criterion, criterion_group, criterion_main};
use lustre_collector::quota::parse as combine_parse;
use memory_benchmarking::{BencherOutput, MemoryUsage, aggregate_samples, trace_memory};
use std::{fs::File, io::Read, sync::mpsc, time::Duration};

pub fn combine_memory(c: &mut Criterion) {
    let (tx, rx) = mpsc::channel();

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
            let routine = move || {
                let mut needle = input.as_str();
                while let Ok((_, e)) = combine_parse().easy_parse(needle) {
                    needle = e;
                }
            };

            let memory_usage: MemoryUsage = trace_memory(routine, Duration::from_millis(100))
                .await
                .as_slice()
                .try_into()
                .expect("Failed to extract memory usage from samples");
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

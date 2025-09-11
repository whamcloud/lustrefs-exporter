use combine::parser::EasyParser;
use criterion::{Criterion, criterion_group, criterion_main};
use lustre_collector::quota::parse as combine_parse;
use std::{fs::File, io::Read, time::Duration};

pub fn combine_perf(c: &mut Criterion) {
    let mut group = c.benchmark_group("parse_benchmarks");

    group.sample_size(10);
    group.measurement_time(Duration::from_secs(90)); // Allow more time

    let mut raw = String::new();
    File::open("benches/quotas.yml")
        .expect("Failed to open file")
        .read_to_string(&mut raw)
        .expect("Failed to read file");

    group.bench_with_input("combine_performance", &raw, |b, input| {
        b.iter(|| {
            let mut needle = input.as_str();
            while let Ok((_, e)) = combine_parse().easy_parse(needle) {
                needle = e;
            }
        })
    });
}

criterion_group!(benches, combine_perf);
criterion_main!(benches);

use combine::parser::EasyParser;
use lustre_collector::quota::parse as combine_parse;
use memory_benchmarking::{BencherOutput, trace_memory};
use std::time::Duration;

pub fn main() {
    let buffer = std::fs::read_to_string("benches/quotas.yml").expect("Failed to read file");
    let needle = buffer.as_str();

    let samples: Vec<_> = (0..100)
        .map(|_| {
            let routine = move || {
                let mut needle = needle;
                while let Ok((_, e)) = combine_parse().easy_parse(needle) {
                    needle = e;
                }
            };

            trace_memory(routine, Duration::from_millis(10))
                .as_slice()
                .try_into()
                .expect("Failed to extract memory usage from samples")
        })
        .collect();

    let bencher_output: BencherOutput = samples.as_slice().into();

    let serialized_metrics = serde_json::to_string_pretty(&bencher_output)
        .expect("Failed to serialize benchmark output.");

    println!("{serialized_metrics}");
}

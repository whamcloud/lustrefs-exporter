use combine::parser::EasyParser;
use lustre_collector::quota::parse as combine_parse;
use memory_benchmarking::{MemoryMetrics, trace_memory};
use std::{collections::HashMap, time::Duration};

pub fn main() {
    let buffer = std::fs::read_to_string("benches/quotas.yml").expect("Failed to read file");
    let needle = buffer.as_str();

    let samples: Vec<_> = (0..100)
        .map(|_| {
            trace_memory(
                move || {
                    let mut needle = needle;
                    while let Ok((_, e)) = combine_parse().easy_parse(needle) {
                        needle = e;
                    }
                },
                Duration::from_millis(10),
            )
            .as_slice()
            .try_into()
            .expect("Failed to extract memory usage from samples")
        })
        .collect();

    let memory_usage: MemoryMetrics = samples.as_slice().into();

    let serialized_metrics =
        serde_json::to_string_pretty(&HashMap::from([("quota_parsing", memory_usage)]))
            .expect("Failed to serialize benchmark output.");

    println!("{serialized_metrics}");
}

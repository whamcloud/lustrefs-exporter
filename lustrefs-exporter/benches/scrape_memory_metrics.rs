// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

mod common;

use common::load_test_concurrent;
use memory_benchmarking::{MemoryMetrics, trace_memory_async};
use std::{collections::HashMap, time::Duration};

pub fn main() {
    let samples: Vec<_> = (0..10)
        .map(|_| {
            trace_memory_async(
                || async {
                    let listener = tokio::net::TcpListener::bind(("0.0.0.0", 12345))
                        .await
                        .expect("Failed to bind to port 12345");

                    axum::serve(listener, lustrefs_exporter::routes::app())
                        .await
                        .expect("Failed to serve app.");
                },
                || async {
                    load_test_concurrent(10, 60).await;
                },
                Duration::from_millis(10),
            )
            .as_slice()
            .try_into()
            .expect("Failed to extract memory usage from samples")
        })
        .collect();

    let serialized_metrics = serde_json::to_string_pretty(&HashMap::from([(
        "scrape_memory_usage",
        MemoryMetrics::from(samples.as_slice()),
    )]))
    .expect("Failed to serialize benchmark output.");

    println!("{serialized_metrics}");
}

// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use commandeer_test::commandeer;
use pprof::ProfilerGuardBuilder;
use serial_test::serial;
use std::time::{Duration, Instant};
use tokio::task::JoinSet;

#[commandeer(Replay, "lctl", "lnetctl")]
#[tokio::test]
#[serial]
#[ignore]
async fn profile_with_flamegraph() {
    // Start CPU profiling
    let cpu_guard = ProfilerGuardBuilder::default()
        .frequency(100)
        .build()
        .unwrap();

    // Start the server
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", 12345))
        .await
        .expect("Failed to bind to port 12345");

    tokio::spawn(async move {
        axum::serve(listener, lustrefs_exporter::routes::app())
            .await
            .expect("Failed to serve app.");
    });

    // Wait for server to be ready
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Run a load test
    let _duration = load_test_concurrent(10, 50).await;

    // Generate CPU flamegraph
    if let Ok(report) = cpu_guard.report().build() {
        let file = std::fs::File::create("cpu-flamegraph.svg").unwrap();
        let mut options = pprof::flamegraph::Options::default();
        report.flamegraph_with_options(&file, &mut options).unwrap();
        println!("CPU flamegraph saved to cpu-flamegraph.svg");
    }
}

// Hit the `/metrics` endpoint to scrape metrics
async fn make_single_request() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let body = reqwest::get("http://0.0.0.0:12345/metrics?jobstats=true")
        .await?
        .text()
        .await?;

    Ok(body)
}

// Use a JoinSet to make `concurrent` requests at a time, waiting for each batch to complete before
// starting the next batch.
async fn load_test_concurrent(concurrency: usize, total_requests: usize) -> Duration {
    let start = Instant::now();

    let mut spawned_requests = 0;
    let mut successful_requests = 0;
    let mut failed_requests = 0;

    let mut join_set = JoinSet::new();

    // Initially spawn `concurrency` requests
    for _ in 0..concurrency {
        join_set.spawn(async move { make_single_request().await });

        spawned_requests += 1;
    }

    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(Ok(_)) => successful_requests += 1,
            Ok(Err(_)) => failed_requests += 1,
            Err(_) => failed_requests += 1,
        }

        // Immediately spawn a new request if there are more to be made.
        if spawned_requests < total_requests {
            join_set.spawn(async move { make_single_request().await });

            spawned_requests += 1;
        }
    }

    let elapsed = start.elapsed();

    println!(
        "Load test completed: {successful_requests} successful, {failed_requests} failed requests in {elapsed:?}.",
    );

    elapsed
}

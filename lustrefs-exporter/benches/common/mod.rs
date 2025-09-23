// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use commandeer_test::commandeer;
use std::time::Duration;
use tokio::{task::JoinSet, time::Instant};

// Create a single request using `oneshot`. This is equivalent to hitting the
// `/scrape` endpoint if the http service was running.
async fn make_single_request() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let body = reqwest::get("http://localhost:12345/metrics?jobstats=true")
        .await?
        .text()
        .await?;

    Ok(body)
}

// Use a JoinSet to make `concurrent` requests at a time, waiting for each batch to complete before
// starting the next batch.
#[commandeer(Replay, "lctl", "lnetctl")]
pub async fn load_test_concurrent(concurrency: usize, total_requests: usize) -> Duration {
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

        // Immediately spawn a new request if there are more to be made
        if spawned_requests < total_requests {
            join_set.spawn(async move { make_single_request().await });

            spawned_requests += 1;
        }
    }

    let elapsed = start.elapsed();

    eprintln!(
        "Load test completed: {successful_requests} successful, {failed_requests} failed requests in {elapsed:?}",
    );

    elapsed
}

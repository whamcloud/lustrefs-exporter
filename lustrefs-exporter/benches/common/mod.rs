// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use axum::{
    Router,
    body::{Body, to_bytes},
    http::Request,
};
use lustrefs_exporter::routes;
use std::{
    env, os::unix::process::ExitStatusExt as _, path::PathBuf, process::{ExitStatus, Output}, sync::Arc, time::Duration
};
use tokio::{task::JoinSet, time::Instant};
use tower::ServiceExt as _;

/// Create a new Axum app with the provided state and a Request
/// to scrape the metrics endpoint.
fn get_app() -> (Request<Body>, Router) {
    let app = routes::app();

    let request = Request::builder()
        .uri("/metrics?jobstats=true")
        .method("GET")
        .body(Body::empty())
        .unwrap();

    (request, app)
}

// Prepare the test environment. This includes:
// 1. Putting the mock lctl binary in the PATH environment variable
// 2. Putting the mock lnetctl binary in the PATH environment variable
pub fn setup_env() {
    let mock_bin = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("benches")
        .join("mock_bins");

    let current_path = env::var("PATH").expect("PATH environment variable not set.");

    let new_path = format!("{current_path}:{}", mock_bin.display());

    unsafe {
        env::set_var("PATH", new_path);
    }
}

// Create a single request using `oneshot`. This is equivalent to hitting the
// `/scrape` endpoint if the http service was running.
async fn make_single_request() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let (request, app) = get_app();
    let resp = app.oneshot(request).await?;
    let body = to_bytes(resp.into_body(), usize::MAX).await?;
    let body_str = std::str::from_utf8(&body)?;

    Ok(body_str.to_string())
}

// Use a JoinSet to make `concurrency` requests at a time, waiting for each batch to complete before
// starting the next batch.
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

    println!(
        "Load test completed: {} successful, {} failed requests in {:?}",
        successful_requests, failed_requests, elapsed
    );

    elapsed
}

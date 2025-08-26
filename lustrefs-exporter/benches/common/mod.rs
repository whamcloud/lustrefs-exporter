// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use axum::{
    Router,
    body::{Body, Bytes, to_bytes},
    http::{Request, header::ACCEPT_ENCODING},
};
use lustrefs_exporter::routes;
use std::time::Duration;
use tokio::{task::JoinSet, time::Instant};
use tower::ServiceExt as _;

/// Create a new Axum app with the provided state and a Request
/// to scrape the metrics endpoint.
fn get_request() -> Request<Body> {
    Request::builder()
        .uri("/metrics?jobstats=true")
        .method("GET")
        .header(ACCEPT_ENCODING, "gzip")
        .body(Body::empty())
        .expect("Failed to build request")
}

// Create a single request using `oneshot`. This is equivalent to hitting the
// `/scrape` endpoint if the http service was running.
async fn make_single_request(
    app: Router,
) -> Result<Bytes, Box<dyn std::error::Error + Send + Sync>> {
    let request = get_request();

    let resp = app.oneshot(request).await?;

    if !resp.status().is_success() {
        return Err(format!("Request failed with status: {}", resp.status()).into());
    }

    let body = to_bytes(resp.into_body(), usize::MAX).await?;

    Ok(body)
}

// Use a JoinSet to make `concurrent` requests at a time, waiting for each batch to complete before
// starting the next batch.
pub async fn load_test_concurrent(concurrency: usize, total_requests: usize) -> Duration {
    let app = routes::app();

    let start = Instant::now();

    let mut spawned_requests = 0;
    let mut successful_requests = 0;
    let mut failed_requests = 0;

    let mut join_set = JoinSet::new();

    // Initially spawn `concurrency` requests
    for _ in 0..concurrency {
        let app = app.clone();

        join_set.spawn(async move { make_single_request(app.clone()).await });

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
            let app = app.clone();

            join_set.spawn(async move { make_single_request(app.clone()).await });

            spawned_requests += 1;
        }
    }

    let elapsed = start.elapsed();

    println!(
        "Load test completed: {successful_requests} successful, {failed_requests} failed requests in {elapsed:?}",
    );

    elapsed
}

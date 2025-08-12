// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use axum::{
    Router,
    body::{Body, to_bytes},
    http::Request,
};
use injectorpp::interface::injector::*;
use lustrefs_exporter::routes;
use std::{
    os::unix::process::ExitStatusExt as _,
    process::{ExitStatus, Output},
    time::Duration,
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

// Create the shared state and wrap it in an Arc so that it can be reused
// across requests without building it each time.
pub fn inject_mocks() {
    let mut injector = InjectorPP::new();

    injector
        .when_called(injectorpp::func!(
            fn(routes::jobstats_metrics_cmd)() -> Result<std::process::Child, std::io::Error>
        ))
        .will_execute(injectorpp::fake!(
            func_type: fn() -> Result<std::process::Child, std::io::Error>,
            returns: std::process::Command::new("cat")
                .arg("../../fixtures/jobstats_only/2.14.0_162.txt")
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()
        ));

    injector
        .when_called_async(injectorpp::async_func!(
            routes::lustre_metrics_output(), Result<std::process::Output, std::io::Error>
        ))
        .will_return_async(injectorpp::async_return!(
            Ok(Output {
                status: ExitStatus::from_raw(0),
                stdout: include_str!("../../../lustre-collector/src/fixtures/valid/lustre-2.14.0_ddn133/2.14.0_ddn133_quota.txt").as_bytes().to_vec(),
                stderr: b"".to_vec(),
            }),
            Result<std::process::Output, std::io::Error>
        ));

    injector
        .when_called_async(injectorpp::async_func!(
            routes::net_show_output(), Result<std::process::Output, std::io::Error>
        ))
        .will_return_async(injectorpp::async_return!(
            Ok(Output {
                status: ExitStatus::from_raw(0),
                stdout: include_str!("../../fixtures/lnetctl_net_show.txt").as_bytes().to_vec(),
                stderr: b"".to_vec(),
            }),
            Result<std::process::Output, std::io::Error>
        ));

    injector
        .when_called_async(injectorpp::async_func!(
            routes::lnet_stats_output(), Result<std::process::Output, std::io::Error>
        ))
        .will_return_async(injectorpp::async_return!(
            Ok(Output {
                status: ExitStatus::from_raw(0),
                stdout: include_str!("../../fixtures/lnetctl_stats.txt").as_bytes().to_vec(),
                stderr: b"".to_vec(),
            }),
            Result<std::process::Output, std::io::Error>
        ));
}

// Create a single request using `oneshot`. This is equivalent to hitting the
// `/scrape` endpoint if the http service was running.
async fn make_single_request() -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    inject_mocks();

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

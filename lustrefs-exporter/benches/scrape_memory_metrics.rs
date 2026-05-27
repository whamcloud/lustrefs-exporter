// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

mod common;

use axum::{
    Router,
    body::Body,
    extract::Query,
    http::{Response, StatusCode, header::CONTENT_TYPE},
};
use common::load_test_concurrent;
use lustrefs_exporter::Error;
use memory_benchmarking::trace_memory_async;
use std::time::Duration;
use tokio::task::JoinSet;
use tower::ServiceBuilder;
use tower_http::compression::CompressionLayer;

use divan::AllocProfiler;

#[global_allocator]
static ALLOC: AllocProfiler = AllocProfiler::system();

#[divan::bench]
pub fn scrape() {
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
    .as_slice();
}

#[divan::bench]
pub fn test_vec() {
    let test: Vec<u8> = Vec::with_capacity(1024 * 1024 * 1024);
}

#[divan::bench]
pub fn test_vec_async() {
    tokio::runtime::Builder::new_multi_thread()
        .enable_time()
        .enable_io()
        .build()
        .expect("Failed to build tokio runtime")
        .block_on(async move {
            let test: Vec<u8> = Vec::with_capacity(1024 * 1024 * 1024);
        })
}

#[divan::bench]
pub fn test_vec_async_join() {
    tokio::runtime::Builder::new_multi_thread()
        .enable_time()
        .enable_io()
        .build()
        .expect("Failed to build tokio runtime")
        .block_on(async move {
            let mut join_set = JoinSet::new();
            for _ in 1..10 {
                join_set.spawn(async move {
                    let test: Vec<u8> = Vec::with_capacity(1024 * 1024 * 1024);
                });
            }

            join_set.join_all().await;
        })
}

#[divan::bench]
pub fn test_vec_axum() {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_time()
        .enable_io()
        .build()
        .expect("Failed to build tokio runtime");

    pub async fn scrape(Query(_): Query<()>) -> Result<Response<Body>, Error> {
        let test: Vec<u8> = Vec::with_capacity(1024 * 1024 * 1024);

        let resp = Response::builder()
            .status(StatusCode::OK)
            .header(
                CONTENT_TYPE,
                "application/openmetrics-text; version=1.0.0; charset=utf-8",
            )
            .body(Body::from("hello"))?;

        Ok(resp)
    }

    let (kill_s, kill_r) = tokio::sync::oneshot::channel();

    let server = async move {
        let listener = tokio::net::TcpListener::bind(("0.0.0.0", 12345))
            .await
            .expect("Failed to bind to port 12345");

        axum::serve(listener, {
            let load_shedder = ServiceBuilder::new().layer(CompressionLayer::new());

            Router::new()
                .route("/", axum::routing::get(scrape))
                .layer(load_shedder)
        })
        .with_graceful_shutdown(async move {
            kill_r.await.ok();
        })
        .await
        .expect("Failed to serve app.");
    };

    let request = async move {
        reqwest::get("http://localhost:12345/")
            .await
            .expect("Request failed")
            .text()
            .await
            .expect("Parsing response failed");

        kill_s.send(()).ok();
    };

    rt.block_on(async move {
        let mut join_set = JoinSet::new();
        join_set.spawn(request);
        join_set.spawn(server);
        join_set.join_all().await
    });
}

fn main() {
    divan::main()
}

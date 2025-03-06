// Copyright (c) 2024 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use axum::{
    body::{Body, Bytes},
    error_handling::HandleErrorLayer,
    extract::Query,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    BoxError, Router,
};
use clap::Parser;
use lustre_collector::{parse_lctl_output, parse_lnetctl_output, parse_lnetctl_stats, parser};
use lustrefs_exporter::{build_lustre_stats, Error};
use serde::Deserialize;
use std::{
    borrow::Cow,
    convert::Infallible,
    io::{self, BufRead, BufReader},
    net::SocketAddr,
};
use tokio::process::Command;
use tokio_stream::{wrappers::ReceiverStream, StreamExt};
use tower::{limit::GlobalConcurrencyLimitLayer, load_shed::LoadShedLayer, ServiceBuilder};

const LUSTREFS_EXPORTER_PORT: &str = "32221";

#[derive(Debug, Parser)]
pub struct CommandOpts {
    /// Port that exporter will listen to
    #[clap(short, long, env = "LUSTREFS_EXPORTER_PORT", default_value = LUSTREFS_EXPORTER_PORT)]
    pub port: u16,
}

async fn handle_error(error: BoxError) -> impl IntoResponse {
    if error.is::<tower::timeout::error::Elapsed>() {
        return (StatusCode::REQUEST_TIMEOUT, Cow::from("request timed out"));
    }

    if error.is::<tower::load_shed::error::Overloaded>() {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Cow::from("service is overloaded, try again later"),
        );
    }

    (
        StatusCode::INTERNAL_SERVER_ERROR,
        Cow::from(format!("Unhandled internal error: {error}")),
    )
}

#[derive(Debug, Deserialize)]
struct Params {
    // Only enable jobstats if "jobstats=true"
    #[serde(default)]
    jobstats: bool,
}

fn create_load_shedder_router() -> Router {
    let load_shedder = ServiceBuilder::new()
        .layer(HandleErrorLayer::new(handle_error))
        .layer(LoadShedLayer::new())
        .layer(GlobalConcurrencyLimitLayer::new(10));

    Router::new()
        .route("/metrics", get(scrape))
        .layer(load_shedder)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();

    let opts = CommandOpts::parse();

    let addr = SocketAddr::from(([0, 0, 0, 0], opts.port));

    tracing::info!("Listening on http://{addr}/metrics");

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", opts.port)).await?;

    let app = create_load_shedder_router();

    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(not(test))]
async fn scrape(Query(params): Query<Params>) -> Result<Response<Body>, Error> {
    let jobstats = if params.jobstats {
        let child = tokio::task::spawn_blocking(move || {
            let child = std::process::Command::new("lctl")
                .arg("get_param")
                .args(["obdfilter.*OST*.job_stats", "mdt.*.job_stats"])
                .stdout(std::process::Stdio::piped())
                .stderr(std::process::Stdio::piped())
                .spawn()?;

            Ok::<_, Error>(child)
        })
        .await?;

        match child {
            Ok(mut child) => {
                let reader = BufReader::with_capacity(
                    128 * 1_024,
                    child.stdout.take().ok_or(io::Error::new(
                        io::ErrorKind::NotFound,
                        "stdout missing for lctl jobstats call.",
                    ))?,
                );

                let reader_stderr = BufReader::new(child.stderr.take().ok_or(io::Error::new(
                    io::ErrorKind::NotFound,
                    "stderr missing for lctl jobstats call.",
                ))?);

                tokio::task::spawn(async move {
                    for line in reader_stderr.lines().map_while(Result::ok) {
                        tracing::debug!("stderr: {}", line);
                    }
                });

                let (_, rx) = lustrefs_exporter::jobstats::jobstats_stream(reader);

                tokio::task::spawn_blocking(move || {
                    if let Err(e) = child.wait() {
                        tracing::debug!("Unexpected error when waiting for child: {e}");
                    }
                });

                let stream = ReceiverStream::new(rx)
                    .map(|x| Bytes::from_iter(x.into_bytes()))
                    .map(Ok::<_, Infallible>);

                Some(stream)
            }
            Err(e) => {
                tracing::debug!("Error while spawning lctl jobstats: {e}");

                None
            }
        }
    } else {
        None
    };

    let mut output = vec![];

    let lctl = Command::new("lctl")
        .arg("get_param")
        .args(parser::params())
        .kill_on_drop(true)
        .output()
        .await?;

    let mut lctl_output = parse_lctl_output(&lctl.stdout)?;

    output.append(&mut lctl_output);

    let lnetctl = Command::new("lnetctl")
        .args(["net", "show", "-v", "4"])
        .kill_on_drop(true)
        .output()
        .await?;

    let lnetctl_stats = std::str::from_utf8(&lnetctl.stdout)?;
    let mut lnetctl_output = parse_lnetctl_output(lnetctl_stats)?;

    output.append(&mut lnetctl_output);

    let lnetctl_stats_output = Command::new("lnetctl")
        .args(["stats", "show"])
        .kill_on_drop(true)
        .output()
        .await?;

    let mut lnetctl_stats_record =
        parse_lnetctl_stats(std::str::from_utf8(&lnetctl_stats_output.stdout)?)?;

    output.append(&mut lnetctl_stats_record);

    let lustre_stats = build_lustre_stats(output);

    let body = if let Some(stream) = jobstats {
        let merged =
            tokio_stream::StreamExt::chain(tokio_stream::once(Ok(lustre_stats.into())), stream);

        Body::from_stream(merged)
    } else {
        tracing::debug!("Jobstats collection disabled");

        Body::from(lustre_stats)
    };

    let response_builder = Response::builder().status(StatusCode::OK);

    let resp = response_builder.body(body)?;

    Ok(resp)
}

#[cfg(test)]
async fn scrape(Query(_params): Query<Params>) -> Result<Response<Body>, Error> {
    // Test concurrency by sleeping the thread to simulate processing.
    tokio::time::sleep(Duration::from_millis(5000)).await;
    let response_builder = Response::builder().status(StatusCode::OK);

    let resp = response_builder.body(Body::from("")).unwrap();
    Ok(resp)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use crate::{build_lustre_stats, create_load_shedder_router};
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use combine::parser::EasyParser;
    use include_dir::{include_dir, Dir};
    use insta::assert_snapshot;
    use lustre_collector::parser::parse;
    use tokio::time::sleep;
    use tower::ServiceExt as _;

    static VALID_FIXTURES: Dir<'_> =
        include_dir!("$CARGO_MANIFEST_DIR/../lustre-collector/src/fixtures/valid/");

    #[test]
    fn test_valid_fixtures() {
        for dir in VALID_FIXTURES.find("*").unwrap() {
            match dir {
                include_dir::DirEntry::Dir(_) => {}
                include_dir::DirEntry::File(file) => {
                    let name = file.path().to_string_lossy();

                    let contents = file.contents_utf8().unwrap();

                    let result = parse()
                        .easy_parse(contents)
                        .map_err(|err| err.map_position(|p| p.translate_position(contents)))
                        .unwrap();

                    let x = build_lustre_stats(result.0);

                    assert_snapshot!(format!("valid_fixture_{name}"), x);
                }
            }
        }
    }

    #[test]
    fn test_stats() {
        let output = include_str!("../fixtures/stats.json");

        let x = serde_json::from_str(output).unwrap();

        let x = build_lustre_stats(x);

        insta::assert_snapshot!(x);
    }

    #[test]
    fn test_lnetctl_stats() {
        let output = include_str!("../fixtures/lnetctl_stats.json");

        let x = serde_json::from_str(output).unwrap();

        let x = build_lustre_stats(x);

        insta::assert_snapshot!(x);
    }

    #[test]
    fn test_lnetctl_stats_mds() {
        let output = include_str!("../fixtures/stats_mds.json");

        let x = serde_json::from_str(output).unwrap();

        let x = build_lustre_stats(x);

        insta::assert_snapshot!(x);
    }

    #[test]
    fn test_host_stats_non_healthy() {
        let output = include_str!("../fixtures/host_stats_non_healthy.json");

        let x = serde_json::from_str(output).unwrap();

        let x = build_lustre_stats(x);

        insta::assert_snapshot!(x);
    }

    #[test]
    fn test_client_stats() {
        let output = include_str!("../fixtures/client.json");

        let x = serde_json::from_str(output).unwrap();

        let x = build_lustre_stats(x);

        insta::assert_snapshot!(x);
    }

    #[tokio::test]
    async fn test_concurrency_limit_10() {
        let app = create_load_shedder_router();

        // Vector to hold all in-flight requests
        let mut in_flight_requests = Vec::new();

        // === Spawn 10 concurrent requests (equal to the concurrency limit) ===
        for _ in 0..10 {
            let cloned_app = app.clone();

            let handle = tokio::spawn(async move {
                cloned_app
                    .oneshot(
                        Request::builder()
                            .uri("/metrics")
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap()
            });

            in_flight_requests.push(handle);
        }

        // Give time for the first 10 requests to engage the concurrency limiter
        sleep(Duration::from_millis(100)).await;

        // === Send one more request that SHOULD be rejected (503) ===
        let extra_app = app.clone();
        let extra_response = extra_app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(
            extra_response.status(),
            StatusCode::SERVICE_UNAVAILABLE,
            "Expected 503 from load shedding"
        );

        // === Join all in-flight requests and assert their responses ===
        for handle in in_flight_requests {
            let response = handle.await.unwrap();
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "Expected 200 OK from in-flight requests"
            );
        }
    }
}

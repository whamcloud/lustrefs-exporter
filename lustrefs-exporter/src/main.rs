// Copyright (c) 2024 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use std::{sync::Arc, time::Duration};

use axum::extract::State;
use axum::routing::get;
use axum::response::IntoResponse;
use axum_streams::*;

use clap::Parser;
use lustre_collector::{parse_lctl_output, parse_lnetctl_output, parse_lnetctl_stats, parser};
use lustrefs_exporter::build_lustre_stats;

use prometheus_client::encoding::{EncodeLabelSet, EncodeLabelValue, EncodeMetric, MetricEncoder};
use prometheus_client::encoding::text::encode;
use prometheus_client::metrics::counter::{Atomic, Counter};
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::MetricType;
use prometheus_client::registry::{self, Registry};

use tokio::net::TcpListener;
use tokio::{
    process::Command,
    sync::Mutex,
    time::{interval, MissedTickBehavior},
};

use tokio_stream::{Stream, StreamExt};

use futures::prelude::*;

const LUSTREFS_EXPORTER_PORT: &str = "32221";

#[derive(Debug)]
struct Options;

#[derive(Debug, Parser)]
pub struct CommandOpts {
    /// Port that exporter will listen to
    #[clap(short, long, env = "LUSTREFS_EXPORTER_PORT", default_value = LUSTREFS_EXPORTER_PORT)]
    pub port: u16,
}

#[derive(Clone)]
pub struct AppState {
    pub registry: Arc<Mutex<Registry>>,
}

async fn handler(axum::extract::State(state): State<AppState>) -> impl IntoResponse {

    use std::time::Instant;

    let now = Instant::now();

    
    let file = std::fs::read_to_string("./lustre-collector/src/fixtures/valid/lustre-2.14.0_ddn125/ds86.txt").unwrap();

    let elapsed = now.elapsed();
    println!("'read_to_string' took: {:.2?}", elapsed);


    let lctl_record = parse_lctl_output(file.as_bytes()).unwrap();
    /* 

    let output = include_str!("../fixtures/jobstats.json");

    let lctl_record = serde_json::from_str(output).unwrap();


    let elapsed = now.elapsed();
    println!("'parse_lctl_output' took: {:.2?}", elapsed);

    */

    let now = Instant::now();

    let stream = build_lustre_stats(lctl_record);

    let elapsed = now.elapsed();
    println!("'build_lustre_stats' took: {:.2?}", elapsed);

    let concatenated_stream = futures::stream::iter(stream.await).flatten();

    StreamBodyAsOptions::new()
        .content_type(HttpHeaderValue::from_static("text/plain; charset=utf-8")).text(concatenated_stream)

}



#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    tracing_subscriber::fmt::init();
    let opts = CommandOpts::parse();

    let registry = Arc::new(Mutex::new(<Registry>::default()));

    let state = AppState { registry: registry.clone() };

    let app = axum::Router::new()
        .route("/metrics", get(handler)).with_state(state);

    let listener = TcpListener::bind(("0.0.0.0", opts.port)).await?;

    axum::serve(listener, app).await.unwrap();

    Ok(())

    
}

#[cfg(test)]
mod tests {
    use crate::build_lustre_stats;
    use combine::parser::EasyParser;
    use include_dir::{include_dir, Dir};
    use insta::assert_snapshot;
    use lustre_collector::parser::parse;

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
    fn test_jobstats() {
        let output = include_str!("../fixtures/jobstats.json");

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
}

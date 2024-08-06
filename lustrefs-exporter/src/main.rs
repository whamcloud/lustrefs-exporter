// Copyright (c) 2024 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use axum::{
    body::{Body, Bytes},
    http::StatusCode,
    response::Response,
    routing::get,
    Router,
};
use clap::Parser;
use lustre_collector::{parse_lctl_output, parse_lnetctl_output, parse_lnetctl_stats, parser};
use lustrefs_exporter::{build_lustre_stats, Error};
use std::{
    convert::Infallible,
    io::{self, BufReader},
    net::SocketAddr,
};
use tokio::process::Command;
use tokio_stream::{wrappers::ReceiverStream, StreamExt};

const LUSTREFS_EXPORTER_PORT: &str = "32221";

#[derive(Debug, Parser)]
pub struct CommandOpts {
    /// Port that exporter will listen to
    #[clap(short, long, env = "LUSTREFS_EXPORTER_PORT", default_value = LUSTREFS_EXPORTER_PORT)]
    pub port: u16,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();

    let opts = CommandOpts::parse();

    let addr = SocketAddr::from(([0, 0, 0, 0], opts.port));

    tracing::info!("Listening on http://{addr}/metrics");

    let listener = tokio::net::TcpListener::bind(("0.0.0.0", opts.port)).await?;

    let app = Router::new().route("/metrics", get(scrape));

    axum::serve(listener, app).await?;

    Ok(())
}

async fn scrape() -> Result<Response<Body>, Error> {
    let mut output = vec![];

    let lctl = Command::new("lctl")
        .arg("get_param")
        .args(parser::params_no_jobstats())
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

    let reader = tokio::task::spawn_blocking(move || {
        let mut lctl_jobstats = std::process::Command::new("lctl")
            .arg("get_param")
            .args(["obdfilter.*OST*.job_stats", "mdt.*.job_stats"])
            .stdout(std::process::Stdio::piped())
            .spawn()?;

        let reader = BufReader::with_capacity(
            128 * 1_024,
            lctl_jobstats.stdout.take().ok_or(io::Error::new(
                io::ErrorKind::NotFound,
                "stdout missing for lctl jobstats call.",
            ))?,
        );

        Ok::<_, Error>(reader)
    })
    .await??;

    let (fut, rx) = lustrefs_exporter::jobstats::jobstats_stream(reader);

    let stream = ReceiverStream::new(rx)
        .map(|x| Bytes::from_iter(x.into_bytes()))
        .map(Ok);

    fut.await??;

    let lustre_stats = Ok::<_, Infallible>(build_lustre_stats(output).into());

    let merged = tokio_stream::StreamExt::merge(tokio_stream::once(lustre_stats), stream);

    let s = Body::from_stream(merged);

    let response_builder = Response::builder().status(StatusCode::OK);

    let resp = response_builder.body(s)?;

    Ok(resp)
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

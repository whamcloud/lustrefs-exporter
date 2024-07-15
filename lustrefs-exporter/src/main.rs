// Copyright (c) 2024 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use std::{sync::Arc, time::Duration};

use clap::Parser;
use lustre_collector::{parse_lctl_output, parse_lnetctl_output, parse_lnetctl_stats, parser};
use lustrefs_exporter::build_lustre_stats;
use prometheus_exporter_base::prelude::*;

use tokio::{
    process::Command,
    sync::Mutex,
    time::{interval, MissedTickBehavior},
};

const LUSTREFS_EXPORTER_PORT: &str = "32221";

#[derive(Debug)]
struct Options;

#[derive(Debug, Parser)]
pub struct CommandOpts {
    /// Port that exporter will listen to
    #[clap(short, long, env = "LUSTREFS_EXPORTER_PORT", default_value = LUSTREFS_EXPORTER_PORT)]
    pub port: u16,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    let opts = CommandOpts::parse();

    let server_opts = ServerOptions {
        addr: ([0, 0, 0, 0], opts.port).into(),
        authorization: Authorization::None,
    };

    let mtx_record = Arc::new(Mutex::new(None));
    let mtx_record2 = Arc::clone(&mtx_record);

    let mut ticker = interval(Duration::from_secs(10));
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    tokio::spawn(async move {
        loop {
            ticker.tick().await;

            let lctl = match Command::new("lctl")
                .arg("get_param")
                .args(parser::params_jobstats_only())
                .kill_on_drop(true)
                .output()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    tracing::debug!("Failed to retrieve jobstats parameters. {e}");
                    continue;
                }
            };

            // Offload CPU-intensive parsing to a blocking task
            let parsed_result =
                tokio::task::spawn_blocking(move || parse_lctl_output(&lctl.stdout)).await;

            match parsed_result {
                Ok(Ok(r)) => {
                    let stat = build_lustre_stats(r);
                    let mut lock = mtx_record.lock().await;
                    *lock = Some(stat);
                }
                Ok(Err(e)) => {
                    tracing::debug!("Failed to parse jobstats information. {e}");
                    continue;
                }
                Err(e) => {
                    tracing::debug!("Failed to execute parse_lctl_output in blocking task. {e}");
                    continue;
                }
            }
        }
    });

    render_prometheus(server_opts, Options, |request, options| async move {
        tracing::debug!(?request, ?options);

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

        let lustre_stats = build_lustre_stats(output);

        let jobstats = { mtx_record2.lock().await.clone() };

        match jobstats {
            Some(jobstats) => Ok([lustre_stats, jobstats].join("\n")),
            None => Ok(lustre_stats),
        }
    })
    .await;
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

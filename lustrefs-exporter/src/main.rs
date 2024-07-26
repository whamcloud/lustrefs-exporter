// Copyright (c) 2024 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use std::{sync::Arc, time::Duration};

use clap::Parser;
use lustre_collector::{parse_lctl_output, parse_lnetctl_output, parse_lnetctl_stats, parser};
use lustrefs_exporter::{build_lustre_stats, build_lustre_stats_new};
use moka::future::Cache;
use prometheus_exporter_base::prelude::*;
use tokio::{
    process::Command,
    time::{interval, MissedTickBehavior},
};

const LUSTREFS_EXPORTER_PORT: &str = "32221";
const JOBSTAT_ENTRY: &str = "JOBSTATS";

#[derive(Debug)]
struct Options;

#[derive(Debug, Parser)]
pub struct CommandOpts {
    /// Port that exporter will listen to
    #[clap(short, long, env = "LUSTREFS_EXPORTER_PORT", default_value = LUSTREFS_EXPORTER_PORT)]
    pub port: u16,
    /// Disable jobstats processing
    #[clap(short, long, env = "LUSTREFS_EXPORTER_JOBSTATS")]
    pub disable_jobstats: bool,
    /// Disable jobstats processing
    #[clap(short, long)]
    pub disable_cache: bool,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let file = std::fs::read_to_string(
        "./lustre-collector/src/fixtures/valid/lustre-2.14.0_ddn125/ds86.txt",
    )
    .unwrap();

    let lctl_record = parse_lctl_output(file.as_bytes()).unwrap();

    let lustre_stats = build_lustre_stats_new(&lctl_record);

    // println!("{lustre_stats}");
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

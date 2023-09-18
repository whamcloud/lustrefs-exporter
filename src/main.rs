// Copyright (c) 2023 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use lustre_collector::{parse_lctl_output, parse_lnetctl_output, parse_lnetctl_stats, parser};
use lustrefs_exporter::build_lustre_stats;
use prometheus_exporter_base::prelude::*;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::process::Command;

#[derive(Debug)]
struct Options;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let server_opts = ServerOptions {
        addr: ([0, 0, 0, 0], 32221).into(),
        authorization: Authorization::None,
    };

    println!("starting exporter on {}", server_opts.addr);

    render_prometheus(server_opts, Options, |request, options| async move {
        tracing::debug!(?request, ?options);

        let time = SystemTime::now().duration_since(UNIX_EPOCH)?;

        let mut output = vec![];

        let lctl = Command::new("lctl")
            .arg("get_param")
            .args(parser::params())
            .kill_on_drop(true)
            .output()
            .await?;

        let mut lctl_output = parse_lctl_output(&lctl.stdout)?;

        output.append(&mut lctl_output);

        let lnetctl = Command::new("sudo")
            .args(["/usr/sbin/lnetctl", "net", "show", "-v", "4"])
            .kill_on_drop(true)
            .output()
            .await?;

        let lnetctl_stats = std::str::from_utf8(&lnetctl.stdout)?;
        let mut lnetctl_output = parse_lnetctl_output(lnetctl_stats)?;

        output.append(&mut lnetctl_output);

        let lnetctl_stats_output = Command::new("sudo")
            .args(["/usr/sbin/lnetctl", "stats", "show"])
            .kill_on_drop(true)
            .output()
            .await?;

        let mut lnetctl_stats_record =
            parse_lnetctl_stats(std::str::from_utf8(&lnetctl_stats_output.stdout)?)?;

        output.append(&mut lnetctl_stats_record);

        Ok(build_lustre_stats(output, time))
    })
    .await;
}

#[cfg(test)]
mod tests {
    use crate::build_lustre_stats;
    use std::time::UNIX_EPOCH;

    #[test]
    fn test_stats() {
        let output = include_str!("../fixtures/stats.json");

        let x = serde_json::from_str(output).unwrap();

        let x = build_lustre_stats(x, UNIX_EPOCH.duration_since(UNIX_EPOCH).unwrap());

        insta::assert_display_snapshot!(x);
    }
    #[test]
    fn test_jobstats() {
        let output = include_str!("../fixtures/jobstats.json");

        let x = serde_json::from_str(output).unwrap();

        let x = build_lustre_stats(x, UNIX_EPOCH.duration_since(UNIX_EPOCH).unwrap());

        insta::assert_display_snapshot!(x);
    }
    #[test]
    fn test_lnetctl_stats() {
        let output = include_str!("../fixtures/lnetctl_stats.json");

        let x = serde_json::from_str(output).unwrap();

        let x = build_lustre_stats(x, UNIX_EPOCH.duration_since(UNIX_EPOCH).unwrap());

        insta::assert_display_snapshot!(x);
    }
}

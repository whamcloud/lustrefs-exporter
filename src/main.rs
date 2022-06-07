// Copyright (c) 2022 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use lustre_collector::{parse_lctl_output, parser};
use lustrefs_exporter::build_lustre_stats;
use prometheus_exporter_base::prelude::*;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::process::Command;

#[derive(Debug)]
struct Options;

#[tokio::main]
async fn main() {
    let addr = ([0, 0, 0, 0], 32221).into();

    println!("starting exporter on {addr}");

    render_prometheus(addr, Options, |request, options| async move {
        println!("in our render_prometheus(request == {request:?}, options == {options:?})");

        let output = Command::new("lctl")
            .arg("get_param")
            .args(parser::params())
            .kill_on_drop(true)
            .output()
            .await?;

        let time = SystemTime::now().duration_since(UNIX_EPOCH)?;

        let lctl_output = parse_lctl_output(&output.stdout)?;

        Ok(build_lustre_stats(lctl_output, time))
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
}

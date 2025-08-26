// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use axum::{Router, error_handling::HandleErrorLayer, routing::get};
use clap::Parser;
use lustre_collector::parser;
use lustrefs_exporter::{
    Error,
    routes::{handle_error, scrape},
};
use std::net::SocketAddr;
use tokio::process::Command;
use tower::ServiceBuilder;

const LUSTREFS_EXPORTER_PORT: &str = "32221";

#[derive(Debug, Parser)]
pub struct CommandOpts {
    /// Port that exporter will listen to
    #[clap(short, long, env = "LUSTREFS_EXPORTER_PORT", default_value = LUSTREFS_EXPORTER_PORT)]
    pub port: u16,

    /// Dump stats as raw string and exit
    #[clap(long, hide = true)]
    pub dump: bool,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();

    let opts = CommandOpts::parse();

    if opts.dump {
        println!("# Dumping lctl get_param output");
        let lctl = Command::new("lctl")
            .arg("get_param")
            .args(parser::params())
            .kill_on_drop(true)
            .output()
            .await?;
        println!("{}", std::str::from_utf8(&lctl.stdout)?);

        println!("# Dumping lctl get_param jobstats output");
        let lctl = Command::new("lctl")
            .arg("get_param")
            .args(["obdfilter.*OST*.job_stats", "mdt.*.job_stats"])
            .kill_on_drop(true)
            .output()
            .await?;
        println!("{}", std::str::from_utf8(&lctl.stdout)?);

        println!("# Dumping lnetctl net show output");
        let lnetctl = Command::new("lnetctl")
            .args(["net", "show", "-v", "4"])
            .kill_on_drop(true)
            .output()
            .await?;

        println!("{}", std::str::from_utf8(&lnetctl.stdout)?);

        println!("# Dumping lnetctl stats show output");
        let lnetctl_stats_output = Command::new("lnetctl")
            .args(["stats", "show"])
            .kill_on_drop(true)
            .output()
            .await?;
        println!("{}", std::str::from_utf8(&lnetctl_stats_output.stdout)?);
    } else {
        let addr = SocketAddr::from(([0, 0, 0, 0], opts.port));

        tracing::info!("Listening on http://{addr}/metrics");

        let listener = tokio::net::TcpListener::bind(("0.0.0.0", opts.port)).await?;

        let load_shedder = ServiceBuilder::new()
            .layer(HandleErrorLayer::new(handle_error))
            .load_shed()
            .concurrency_limit(10); // Max 10 concurrent scrape

        let app = Router::new()
            .route("/metrics", get(scrape))
            .layer(load_shedder);

        axum::serve(listener, app).await?;
    }

    Ok(())
}

// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use clap::Parser;
use lustrefs_exporter::{
    Error,
    client::ClientLabels,
    dump_stats,
    routes::{ExporterConfig, app},
};
use std::net::SocketAddr;

const LUSTREFS_EXPORTER_PORT: &str = "32221";

#[derive(Debug, Parser)]
pub struct CommandOpts {
    /// Port that exporter will listen to
    #[clap(short, long, env = "LUSTREFS_EXPORTER_PORT", default_value = LUSTREFS_EXPORTER_PORT)]
    pub port: u16,

    /// Dump stats as raw string and exit
    #[clap(long, hide = true)]
    dump: bool,

    /// Sum the osc and mdc client metrics per filesystem instead of per
    /// target; lustre_osc_state becomes the number of OSCs in each state.
    #[clap(
        long,
        env = "LUSTREFS_EXPORTER_AGGREGATE_CLIENT_METRICS",
        value_parser = clap::builder::BoolishValueParser::new()
    )]
    aggregate_client_metrics: bool,
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();

    let opts = CommandOpts::parse();

    if opts.dump {
        dump_stats().await?;
    } else {
        let addr = SocketAddr::from(([0, 0, 0, 0], opts.port));

        tracing::info!("Listening on http://{addr}/metrics");

        let listener = tokio::net::TcpListener::bind(("0.0.0.0", opts.port)).await?;

        let config = ExporterConfig {
            client_labels: if opts.aggregate_client_metrics {
                ClientLabels::ByFilesystem
            } else {
                ClientLabels::PerTarget
            },
        };

        axum::serve(listener, app(config)).await?;
    }

    Ok(())
}

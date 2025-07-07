// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use clap::Parser;
use lustrefs_exporter::{
    Error,
    routes::{
        AppState, app, jobstats_metrics_cmd, lnet_stats_output, lustre_metrics_output,
        net_show_output,
    },
};
use std::{collections::HashMap, net::SocketAddr};

const LUSTREFS_EXPORTER_PORT: &str = "32221";

#[derive(Debug, Parser)]
pub struct CommandOpts {
    /// Port that exporter will listen to
    #[clap(short, long, env = "LUSTREFS_EXPORTER_PORT", default_value = LUSTREFS_EXPORTER_PORT)]
    pub port: u16,

    /// Dump stats as raw string and exit
    #[clap(long, hide = true)]
    dump: bool,
}

/// Dumps Lustre filesystem statistics to stdout
///
/// This function executes several Lustre commands and prints their raw output:
/// - `lctl get_param` with all standard parameters from the parser
/// - `lctl get_param` for jobstats (OST and MDT job statistics)
/// - `lnetctl net show -v 4` for network configuration details
/// - `lnetctl stats show` for network statistics
///
/// # Arguments
/// * `cmd_hdl` - Command handler implementing `RemoteCmd` trait for executing commands
///
/// # Returns
/// * `Ok(())` on successful execution of all commands
/// * `Err(Error)` if any command fails or output cannot be converted to UTF-8
///
/// # Example
/// ```rust
/// use lustrefs_exporter::remote_cmd::LocalCmd;
///
/// dump_stats(&LocalCmd).await?;
/// ```
async fn dump_stats(env_vars: &HashMap<&'static str, String>) -> Result<(), Error> {
    println!("# Dumping lctl get_param output");

    let mut lctl = lustre_metrics_output(env_vars);

    let lctl = lctl.output().await?;

    println!("{}", std::str::from_utf8(&lctl.stdout)?);

    println!("# Dumping lctl get_param jobstats output");

    let mut lctl = jobstats_metrics_cmd(env_vars);

    let lctl = tokio::task::spawn_blocking(move || lctl.output()).await??;

    println!("{}", std::str::from_utf8(&lctl.stdout)?);

    println!("# Dumping lnetctl net show output");

    let mut lnetctl = net_show_output(env_vars);

    let lnetctl = lnetctl.output().await?;

    println!("{}", std::str::from_utf8(&lnetctl.stdout)?);

    println!("# Dumping lnetctl stats show output");

    let mut lnetctl_stats_output = lnet_stats_output(env_vars);

    let lnetctl_stats_output = lnetctl_stats_output.output().await?;

    println!("{}", std::str::from_utf8(&lnetctl_stats_output.stdout)?);

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    tracing_subscriber::fmt::init();

    let app_state = AppState {
        env_vars: Default::default(),
    };

    let opts = CommandOpts::parse();

    if opts.dump {
        dump_stats(&app_state.env_vars).await?;
    } else {
        let addr = SocketAddr::from(([0, 0, 0, 0], opts.port));

        tracing::info!("Listening on http://{addr}/metrics");

        let listener = tokio::net::TcpListener::bind(("0.0.0.0", opts.port)).await?;

        axum::serve(listener, app(app_state))
            .with_graceful_shutdown(async {
                tokio::signal::ctrl_c().await.ok();
            })
            .await?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use lustrefs_exporter::TestEnv;

    use crate::dump_stats;

    #[tokio::test]
    async fn test_dump_stats() {
        let test_env = TestEnv::default();

        dump_stats(&test_env.vars()).await.unwrap();
    }
}

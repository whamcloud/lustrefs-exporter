// Copyright (c) 2024 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

pub mod brw_stats;
pub mod host;
pub mod jobstats;
pub mod llite;
pub mod lnet;
pub mod metrics;
pub mod quota;
pub mod routes;
pub mod service;
pub mod stats;

use crate::routes::{
    jobstats_metrics_cmd, lnet_stats_output, lustre_metrics_output, net_show_output,
};
use axum::{
    http::{self, StatusCode},
    response::{IntoResponse, Response},
};
use lustre_collector::{LustreCollectorError, TargetVariant};
use prometheus_client::metrics::family::Family as PrometheusFamily;

pub type LabelContainer = Vec<(&'static str, String)>;
pub type Family<T> = PrometheusFamily<LabelContainer, T>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Fmt(#[from] std::fmt::Error),
    #[error(transparent)]
    Http(#[from] http::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    LustreCollector(#[from] LustreCollectorError),
    #[error("Could not find match for {0} in {1}")]
    NoCap(&'static str, String),
    #[error(transparent)]
    OneshotReceive(#[from] tokio::sync::oneshot::error::RecvError),
    #[error("{0}")]
    Prometheus(std::fmt::Error),
    #[error(transparent)]
    TaskJoin(#[from] tokio::task::JoinError),
    #[error(transparent)]
    Utf8(#[from] std::str::Utf8Error),
}

impl IntoResponse for Error {
    fn into_response(self) -> Response {
        tracing::warn!("{self}");

        StatusCode::INTERNAL_SERVER_ERROR.into_response()
    }
}

trait LabelProm {
    fn to_prom_label(&self) -> &'static str;
}

impl LabelProm for TargetVariant {
    fn to_prom_label(&self) -> &'static str {
        match self {
            TargetVariant::Ost => "ost",
            TargetVariant::Mgt => "mgt",
            TargetVariant::Mdt => "mdt",
        }
    }
}

/// Dumps Lustre filesystem statistics to stdout
///
/// This function executes several Lustre commands and prints their raw output:
/// - `lctl get_param` with all standard parameters from the parser
/// - `lctl get_param` for jobstats (OST and MDT job statistics)
/// - `lnetctl net show -v 4` for network configuration details
/// - `lnetctl stats show` for network statistics
///
/// # Returns
/// * `Ok(())` on successful execution of all commands
/// * `Err(Error)` if any command fails or output cannot be converted to UTF-8
///
/// # Example
/// ```rust
/// use lustrefs_exporter::dump_stats;
///
/// async fn test_dump_stats() {
///     dump_stats().await.unwrap();
/// }
/// ```
pub async fn dump_stats() -> Result<(), Error> {
    println!("# Dumping lctl get_param output");

    let mut lctl = lustre_metrics_output();

    let lctl = lctl.output().await?;

    println!("{}", std::str::from_utf8(&lctl.stdout)?);

    println!("# Dumping lctl get_param jobstats output");

    let mut lctl = jobstats_metrics_cmd();

    let lctl = tokio::task::spawn_blocking(move || lctl.output()).await??;

    println!("{}", std::str::from_utf8(&lctl.stdout)?);

    println!("# Dumping lnetctl net show output");

    let mut lnetctl = net_show_output();

    let lnetctl = lnetctl.output().await?;

    println!("{}", std::str::from_utf8(&lnetctl.stdout)?);

    println!("# Dumping lnetctl stats show output");

    let mut lnetctl_stats_output = lnet_stats_output();

    let lnetctl_stats_output = lnetctl_stats_output.output().await?;

    println!("{}", std::str::from_utf8(&lnetctl_stats_output.stdout)?);

    Ok(())
}

#[cfg(test)]
pub mod tests {
    use crate::{Error, LabelProm as _, dump_stats};
    use axum::{http::StatusCode, response::IntoResponse as _};
    use commandeer_test::commandeer;
    use lustre_collector::TargetVariant;
    use prometheus_parse::Scrape;
    use serial_test::serial;

    #[test]
    fn test_error_into_response() {
        let error = Error::NoCap("test_param", "test_content".to_string());
        let response = error.into_response();

        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }

    #[test]
    fn test_target_variant_to_prom_label() {
        assert_eq!(TargetVariant::Ost.to_prom_label(), "ost");
        assert_eq!(TargetVariant::Mgt.to_prom_label(), "mgt");
        assert_eq!(TargetVariant::Mdt.to_prom_label(), "mdt");
    }

    #[commandeer(Replay, "lctl", "lnetctl")]
    #[tokio::test]
    #[serial]
    async fn test_dump_stats() {
        dump_stats().await.unwrap();
    }

    pub fn get_scrape(x: String) -> Scrape {
        // According to the Prometheus text exposition format specification,
        // curly braces {} are required even for empty label sets.
        // See: https://prometheus.io/docs/instrumenting/exposition_formats/#text-format-details
        // The format is: metric_name [ "{" label_name "=" `"` label_value `"` ... "}" ] value [ timestamp ]
        // The square brackets indicate the label section is optional, but when present,
        // the curly braces are part of the required syntax, even if no labels exist.
        // Therefore, as an example, "lustre_mem_used_max{} 1611219801" is the correct format,
        // not "lustre_mem_used_max 1611219801". However, `Scrape::parse` will not parse this correctly... So
        // it needs to be removed before parsing. This only affects testing.
        let x = x.replace("{}", "");

        let x = x.lines().map(|x| Ok(x.to_owned()));

        Scrape::parse(x).unwrap()
    }
}

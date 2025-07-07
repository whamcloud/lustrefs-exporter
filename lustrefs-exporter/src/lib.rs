// Copyright (c) 2024 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

pub mod brw_stats;
pub mod host;
pub mod jobstats;
pub mod llite;
pub mod lnet;
pub mod openmetrics;
pub mod quota;
pub mod routes;
pub mod service;
pub mod stats;

use std::{collections::HashMap, env, path::PathBuf};

use axum::{
    http::{self, StatusCode},
    response::{IntoResponse, Response},
};
use lustre_collector::{LustreCollectorError, TargetVariant};
use prometheus_client::metrics::family::Family as PrometheusFamily;

pub type LabelContainer = Vec<(&'static str, String)>;
pub type Family<T> = PrometheusFamily<LabelContainer, T>;

/// Creates a label container by combining provided labels with the default OpenTelemetry scope label.
///
/// This function takes a slice of label tuples and appends a default `otel_scope_name` label
/// with the value "lustre" to create a complete set of labels for Prometheus metrics.
///
/// # Arguments
///
/// * `labels` - A slice of tuples containing label key-value pairs where keys are static strings
///   and values are owned strings
///
/// # Returns
///
/// A `LabelContainer` (vector of label tuples) containing the input labels plus the default
/// OpenTelemetry scope label
///
/// # Examples
///
/// ```
/// use lustrefs_exporter::create_labels;
///
/// let labels = create_labels(&[
///     ("component", "mdt".to_string()),
///     ("target", "fs-MDT0000".to_string()),
/// ]);
/// // Results in: [("component", "mdt"), ("target", "fs-MDT0000"), ("otel_scope_name", "lustre")]
/// ```
pub fn create_labels(labels: &[(&'static str, String)]) -> LabelContainer {
    let mut result = Vec::with_capacity(labels.len() + 1);

    result.extend_from_slice(labels);

    result.push(("otel_scope_name", "lustre".to_string()));

    result
}

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

// Used to mock environment for unit testing and benchmarking
pub struct TestEnv {
    vars: HashMap<&'static str, String>,
}

impl TestEnv {
    pub fn set_var(&mut self, key: &'static str, val: &str) {
        self.vars.insert(key, val.to_string());
    }

    pub fn vars(&self) -> HashMap<&'static str, String> {
        self.vars.clone()
    }
}

impl Default for TestEnv {
    fn default() -> Self {
        let mock_bin_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("mock_bins");
        let current_path = env::var("PATH").unwrap_or_default();

        let path_var = if !current_path.contains(&mock_bin_path.display().to_string()) {
            format!("{current_path}:{}", mock_bin_path.display())
        } else {
            current_path
        };

        Self {
            vars: vec![
                ("PATH", path_var),
                ("JOBSTATS_RESPONSE_FILE", "../fixtures/jobstats_only/2.14.0_164.txt".to_string()),
                ("LUSTRE_RESPONSE_FILE", "../../lustre-collector/src/fixtures/valid/lustre-2.14.0_ddn133/2.14.0_ddn133_quota.txt".to_string()),
                ("NET_SHOW_RESPONSE_FILE", "../fixtures/lnetctl_net_show.txt".to_string()),
                ("NET_STATS_SHOW_RESPONSE_FILE", "../fixtures/lnetctl_stats.txt".to_string())
            ]
                .into_iter()
                .collect::<HashMap<_, _>>()
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Error, LabelProm as _, create_labels};
    use axum::{http::StatusCode, response::IntoResponse};
    use lustre_collector::TargetVariant;

    #[test]
    fn test_create_labels_empty() {
        let result = create_labels(&[]);
        assert_eq!(result, vec![("otel_scope_name", "lustre".to_string())]);
    }

    #[test]
    fn test_create_labels_single() {
        let result = create_labels(&[("component", "mdt".to_string())]);
        assert_eq!(
            result,
            vec![
                ("component", "mdt".to_string()),
                ("otel_scope_name", "lustre".to_string())
            ]
        );
    }

    #[test]
    fn test_create_labels_multiple() {
        let result = create_labels(&[
            ("component", "mdt".to_string()),
            ("target", "fs-MDT0000".to_string()),
            ("operation", "read".to_string()),
        ]);
        assert_eq!(
            result,
            vec![
                ("component", "mdt".to_string()),
                ("target", "fs-MDT0000".to_string()),
                ("operation", "read".to_string()),
                ("otel_scope_name", "lustre".to_string())
            ]
        );
    }

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
}

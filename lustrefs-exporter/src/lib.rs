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
#[cfg(feature = "mock_bin")]
pub mod test_utils;

use axum::{
    http::{self, StatusCode},
    response::{IntoResponse, Response},
};
use lustre_collector::{LustreCollectorError, TargetVariant};
use prometheus_client::metrics::family::Family as PrometheusFamily;

#[cfg(feature = "mock_bin")]
use crate::test_utils::MockCommander;

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

#[cfg(feature = "mock_bin")]
#[derive(Default, strum_macros::AsRefStr)]
pub enum JobstatsMock {
    #[strum(serialize = "fixtures/jobstats_only/2.14.0_162.txt")]
    Lustre2_14_0_162,
    #[default]
    #[strum(serialize = "fixtures/jobstats_only/2.14.0_164.txt")]
    Lustre2_14_0_164,
}

#[cfg(feature = "mock_bin")]
#[derive(Default, strum_macros::AsRefStr)]
pub enum LustreMock {
    #[strum(
        serialize = "../lustre-collector/src/fixtures/valid/lustre-2.14.0_ddn133/2.14.0_ddn133_quota.txt"
    )]
    #[default]
    Lustre2_14_0Ddn133Quota,
    #[strum(serialize = "../lustre-collector/src/fixtures/valid/valid_mds.txt")]
    ValidMds,
}

#[cfg(feature = "mock_bin")]
pub fn create_mock_commander(
    jobstats_mock_file: JobstatsMock,
    lustre_mock_file: LustreMock,
) -> MockCommander {
    use std::path::PathBuf;

    let mock_commander = MockCommander::default();
    mock_commander
        .mock_command("lctl")
        .unwrap()
        .with_args(vec![
            "get_param".to_string(),
            "obdfilter.*OST*.job_stats".to_string(),
            "mdt.*.job_stats".to_string(),
        ])
        .returns_file(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(jobstats_mock_file.as_ref()))
        .unwrap();

    mock_commander
        .mock_command("lctl").unwrap()
        .with_args_string("get_param memused memused_max lnet_memused health_check mdt.*.exports.*.uuid osd-*.*.filesfree osd-*.*.filestotal osd-*.*.fstype osd-*.*.kbytesavail osd-*.*.kbytesfree osd-*.*.kbytestotal osd-*.*.brw_stats osd-*.*.quota_slave.acct_group osd-*.*.quota_slave.acct_user osd-*.*.quota_slave.acct_project mgs.*.mgs.stats mgs.*.mgs.threads_max mgs.*.mgs.threads_min mgs.*.mgs.threads_started mgs.*.num_exports obdfilter.*OST*.stats obdfilter.*OST*.num_exports obdfilter.*OST*.tot_dirty obdfilter.*OST*.tot_granted obdfilter.*OST*.tot_pending obdfilter.*OST*.exports.*.stats ost.OSS.ost.stats ost.OSS.ost_io.stats ost.OSS.ost_create.stats ost.OSS.ost_out.stats ost.OSS.ost_seq.stats mds.MDS.mdt.stats mds.MDS.mdt_fld.stats mds.MDS.mdt_io.stats mds.MDS.mdt_out.stats mds.MDS.mdt_readpage.stats mds.MDS.mdt_seqm.stats mds.MDS.mdt_seqs.stats mds.MDS.mdt_setattr.stats mdt.*.md_stats mdt.*MDT*.num_exports mdt.*MDT*.exports.*.stats ldlm.namespaces.{mdt-,filter-}*.contended_locks ldlm.namespaces.{mdt-,filter-}*.contention_seconds ldlm.namespaces.{mdt-,filter-}*.ctime_age_limit ldlm.namespaces.{mdt-,filter-}*.early_lock_cancel ldlm.namespaces.{mdt-,filter-}*.lock_count ldlm.namespaces.{mdt-,filter-}*.lock_timeouts ldlm.namespaces.{mdt-,filter-}*.lock_unused_count ldlm.namespaces.{mdt-,filter-}*.lru_max_age ldlm.namespaces.{mdt-,filter-}*.lru_size ldlm.namespaces.{mdt-,filter-}*.max_nolock_bytes ldlm.namespaces.{mdt-,filter-}*.max_parallel_ast ldlm.namespaces.{mdt-,filter-}*.resource_count ldlm.services.ldlm_canceld.stats ldlm.services.ldlm_cbd.stats llite.*.stats mdd.*.changelog_users qmt.*.*.glb-usr qmt.*.*.glb-prj qmt.*.*.glb-grp".to_string())
        .returns_file(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(lustre_mock_file.as_ref()))
        .unwrap();

    mock_commander
        .mock_command("lnetctl")
        .unwrap()
        .with_args_string("net show -v 4".to_string())
        .returns_file(
            &PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/lnetctl_net_show.txt"),
        )
        .unwrap();

    mock_commander
        .mock_command("lnetctl")
        .unwrap()
        .with_args_string("stats show".to_string())
        .returns_file(&PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("fixtures/lnetctl_stats.txt"))
        .unwrap();

    mock_commander
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

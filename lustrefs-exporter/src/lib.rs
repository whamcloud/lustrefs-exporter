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
pub mod service;
pub mod stats;

use axum::{
    http::{self, StatusCode},
    response::{IntoResponse, Response},
};
use lustre_collector::{LustreCollectorError, TargetVariant};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error(transparent)]
    Http(#[from] http::Error),
    #[error(transparent)]
    TaskJoin(#[from] tokio::task::JoinError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    LustreCollector(#[from] LustreCollectorError),
    #[error(transparent)]
    Utf8(#[from] std::str::Utf8Error),
    #[error("Could not find match for {0} in {1}")]
    NoCap(&'static str, String),
    #[error(transparent)]
    Otel(#[from] opentelemetry_sdk::metrics::MetricError),
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

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
use opentelemetry_sdk::{metrics::SdkMeterProvider, Resource};
use prometheus::Registry;

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

pub fn init_opentelemetry() -> Result<
    (opentelemetry_sdk::metrics::SdkMeterProvider, Registry),
    opentelemetry_sdk::metrics::MetricError,
> {
    // Build the Prometheus exporter.
    // The metrics will be exposed in the Prometheus format.
    let registry = Registry::new();
    let prometheus_exporter = opentelemetry_prometheus::exporter()
        .with_registry(registry.clone())
        .without_counter_suffixes()
        .build()?;

    let provider = SdkMeterProvider::builder()
        .with_reader(prometheus_exporter)
        .with_resource(
            Resource::builder()
                .with_service_name("lustrefs-exporter")
                .build(),
        )
        .build();

    // Set the global MeterProvider to the one created above.
    // This will make all meters created with `global::meter()` use the above MeterProvider.
    opentelemetry::global::set_meter_provider(provider.clone());

    Ok((provider, registry))
}

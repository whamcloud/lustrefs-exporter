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
use brw_stats::build_target_stats;
use host::build_host_stats;
use lnet::build_lnet_stats;
use lustre_collector::{
    HostStat, LNetStat, LNetStatGlobal, LustreCollectorError, Record, TargetStat, TargetVariant,
};
use num_traits::Num;
use prometheus_exporter_base::{prelude::*, Yes};
use service::build_service_stats;
use std::{
    collections::{BTreeMap, HashSet},
    fmt,
    ops::Deref,
};

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

#[derive(Debug, Clone, Copy)]
struct Metric {
    name: &'static str,
    help: &'static str,
    r#type: MetricType,
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

impl From<Metric> for PrometheusMetric<'_> {
    fn from(x: Metric) -> Self {
        PrometheusMetric::build()
            .with_name(x.name)
            .with_help(x.help)
            .with_metric_type(x.r#type)
            .build()
    }
}

trait ToMetricInst<T>
where
    T: Num + fmt::Display + fmt::Debug + Copy,
{
    fn to_metric_inst(&self) -> PrometheusInstance<'_, T, Yes>;
}

impl<T> ToMetricInst<T> for TargetStat<T>
where
    T: Num + fmt::Display + fmt::Debug + Copy,
{
    fn to_metric_inst(&self) -> PrometheusInstance<'_, T, Yes> {
        PrometheusInstance::new()
            .with_label("component", self.kind.to_prom_label())
            .with_label("target", self.target.deref())
            .with_value(self.value)
    }
}

impl<T> ToMetricInst<T> for LNetStat<T>
where
    T: Num + fmt::Display + fmt::Debug + Copy,
{
    fn to_metric_inst(&self) -> PrometheusInstance<'_, T, Yes> {
        PrometheusInstance::new()
            .with_label("nid", self.nid.deref())
            .with_value(self.value)
    }
}

impl<T> ToMetricInst<T> for LNetStatGlobal<T>
where
    T: Num + fmt::Display + fmt::Debug + Copy,
{
    fn to_metric_inst(&self) -> PrometheusInstance<'_, T, Yes> {
        PrometheusInstance::new().with_value(self.value)
    }
}

impl<T> ToMetricInst<T> for HostStat<T>
where
    T: Num + fmt::Display + fmt::Debug + Copy,
{
    fn to_metric_inst(&self) -> PrometheusInstance<'_, T, Yes> {
        PrometheusInstance::new().with_value(self.value)
    }
}

trait Name {
    fn name(&self) -> &'static str;
}

impl Name for Metric {
    fn name(&self) -> &'static str {
        self.name
    }
}

trait StatsMapExt {
    fn get_mut_metric<T: Name + Into<PrometheusMetric<'static>>>(
        &mut self,
        x: T,
    ) -> &mut PrometheusMetric<'static>;
}

impl StatsMapExt for BTreeMap<&'static str, PrometheusMetric<'static>> {
    fn get_mut_metric<T: Name + Into<PrometheusMetric<'static>>>(
        &mut self,
        x: T,
    ) -> &mut PrometheusMetric<'static> {
        self.entry(x.name()).or_insert_with(|| x.into())
    }
}

pub fn build_lustre_stats(output: Vec<Record>) -> String {
    let mut stats_map = BTreeMap::new();

    let mut set = HashSet::new();
    for x in output {
        match x {
            lustre_collector::Record::Host(x) => {
                build_host_stats(x, &mut stats_map);
            }
            lustre_collector::Record::Node(_) => {}
            lustre_collector::Record::LNetStat(x) => {
                build_lnet_stats(x, &mut stats_map);
            }
            lustre_collector::Record::Target(x) => {
                build_target_stats(x, &mut stats_map, &mut set);
            }
            lustre_collector::Record::LustreService(x) => {
                build_service_stats(x, &mut stats_map);
            }
        }
    }

    stats_map
        .values()
        .map(|x| x.render())
        .collect::<Vec<_>>()
        .join("\n")
}

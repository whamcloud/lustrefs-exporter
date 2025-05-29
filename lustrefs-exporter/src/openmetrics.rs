// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    brw_stats::opentelemetry::{OpenTelemetryMetricsBrw, build_target_stats},
    host::opentelemetry::{OpenTelemetryMetricsHost, build_host_stats},
    llite::opentelemetry::OpenTelemetryMetricsLlite,
    lnet::opentelemetry::{OpenTelemetryMetricsLnet, build_lnet_stats},
    quota::opentelemetry::OpenTelemetryMetricsQuota,
    service::opentelemetry::{OpenTelemetryMetricsService, build_service_stats},
    stats::opentelemetry::OpenTelemetryMetricsStats,
};
use lustre_collector::Record;
use opentelemetry::metrics::Meter;
use std::collections::HashSet;

#[derive(Debug)]
pub struct OpenTelemetryMetrics {
    pub quota: OpenTelemetryMetricsQuota,
    pub host: OpenTelemetryMetricsHost,
    pub service: OpenTelemetryMetricsService,
    pub brw: OpenTelemetryMetricsBrw,
    pub llite: OpenTelemetryMetricsLlite,
    pub lnet: OpenTelemetryMetricsLnet,
    pub stats: OpenTelemetryMetricsStats,
    pub export: OpenTelemetryMetricsStats,
    pub mds: OpenTelemetryMetricsStats, // Reusing the Stats structure for MDS metrics
}

impl OpenTelemetryMetrics {
    pub fn new(meter: Meter) -> Self {
        OpenTelemetryMetrics {
            quota: OpenTelemetryMetricsQuota::new(&meter),
            host: OpenTelemetryMetricsHost::new(&meter),
            service: OpenTelemetryMetricsService::new(&meter),
            brw: OpenTelemetryMetricsBrw::new(&meter),
            llite: OpenTelemetryMetricsLlite::new(&meter),
            lnet: OpenTelemetryMetricsLnet::new(&meter),
            stats: OpenTelemetryMetricsStats::new(&meter),
            export: OpenTelemetryMetricsStats::new(&meter),
            mds: OpenTelemetryMetricsStats::new(&meter), // Reusing the Stats structure for MDS metrics
        }
    }
}

pub fn build_lustre_stats(output: &Vec<Record>, otel: OpenTelemetryMetrics) {
    // This set is used to store the possible duplicate target stats
    let mut set = HashSet::new();
    for x in output {
        match x {
            lustre_collector::Record::Host(x) => {
                build_host_stats(x, &otel.host);
            }
            lustre_collector::Record::LNetStat(x) => {
                build_lnet_stats(x, &otel.lnet);
            }
            lustre_collector::Record::Target(x) => {
                build_target_stats(x, &otel, &mut set);
            }
            lustre_collector::Record::LustreService(x) => {
                build_service_stats(x, &otel.service);
            }
            _ => {}
        }
    }
}

// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    Family,
    brw_stats::{BrwStatsMetrics, build_target_stats},
    host::{HostMetrics, build_host_stats},
    llite::LliteMetrics,
    lnet::{LNetMetrics, build_lnet_stats},
    quota::QuotaMetrics,
    service::{ServiceMetrics, build_service_stats},
    stats::StatsMetrics,
};
use lustre_collector::Record;
use prometheus_client::{metrics::gauge::Gauge, registry::Registry};
use std::{collections::HashSet, sync::atomic::AtomicU64};

#[derive(Debug, Default)]
pub struct Metrics {
    pub host: HostMetrics,
    pub quota: QuotaMetrics,
    pub service: ServiceMetrics,
    pub brw: BrwStatsMetrics,
    pub llite: LliteMetrics,
    pub lnet: LNetMetrics,
    pub stats: StatsMetrics,
    pub export: StatsMetrics,
    pub mds: StatsMetrics, // Reusing the Stats structure for MDS metrics
    target_info: Family<Gauge<u64, AtomicU64>>,
}

impl Metrics {
    pub fn register_metric(&self, registry: &mut Registry) {
        self.host.register_metric(registry);
        self.quota.register_metric(registry);
        self.service.register_metric(registry);
        self.brw.register_metric(registry);
        self.llite.register_metric(registry);
        self.lnet.register_metric(registry);
        self.stats.register_metric(registry);
        self.export.register_metric(registry);
        self.mds.register_metric(registry);

        // prometheus_client does not automatically include the `target_info` metric.
        // Add it manually.
        registry.register("target_info", "Target metadata", self.target_info.clone());
    }
}

pub fn build_lustre_stats(output: &Vec<Record>, metrics: &mut Metrics) {
    // This set is used to store the possible duplicate target stats
    let mut set = HashSet::new();

    for x in output {
        match x {
            lustre_collector::Record::Host(x) => {
                build_host_stats(x, &mut metrics.host);
            }
            lustre_collector::Record::LNetStat(x) => {
                build_lnet_stats(x, &mut metrics.lnet);
            }
            lustre_collector::Record::Target(x) => {
                build_target_stats(x, metrics, &mut set);
            }
            lustre_collector::Record::LustreService(x) => {
                build_service_stats(x, &mut metrics.service);
            }
            _ => {}
        }
    }
}

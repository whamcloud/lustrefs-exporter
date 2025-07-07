// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{Family, create_labels};
use lustre_collector::HostStats;
use prometheus_client::{
    metrics::{counter::Counter, gauge::Gauge},
    registry::Registry,
};
use std::{ops::Deref, sync::atomic::AtomicU64};

#[derive(Debug, Default)]
pub struct HostMetrics {
    lustre_targets_healthy: Family<Gauge<u64, AtomicU64>>,
    lnet_mem_used: Family<Gauge<u64, AtomicU64>>,
    mem_used: Family<Gauge<u64, AtomicU64>>,
    mem_used_max: Family<Counter<u64, AtomicU64>>,
}

impl HostMetrics {
    pub fn register_metric(&self, registry: &mut Registry) {
        registry.register(
            "lustre_health_healthy",
            "Indicates whether the Lustre server is healthy or not. 1 is healthy, 0 is unhealthy",
            self.lustre_targets_healthy.clone(),
        );

        registry.register(
            "lustre_lnet_mem_used",
            "Gives information about Lustre LNet memory usage",
            self.lnet_mem_used.clone(),
        );

        registry.register(
            "lustre_mem_used",
            "Gives information about Lustre memory usage",
            self.mem_used.clone(),
        );

        registry.register_without_auto_suffix(
            "lustre_mem_used_max",
            "Gives information about Lustre maximum memory usage",
            self.mem_used_max.clone(),
        );
    }
}

pub fn build_host_stats(stats: &HostStats, metrics: &mut HostMetrics) {
    match stats {
        HostStats::HealthCheck(x) => {
            let healthy = x.value.healthy;

            metrics
                .lustre_targets_healthy
                .get_or_create(&create_labels(&[]))
                .set(if healthy { 1 } else { 0 });

            for target in &x.value.targets {
                metrics
                    .lustre_targets_healthy
                    .get_or_create(&create_labels(&[("target", target.deref().to_string())]))
                    .set(if healthy { 1 } else { 0 });
            }
        }
        HostStats::LNetMemUsed(x) => {
            metrics
                .lnet_mem_used
                .get_or_create(&create_labels(&[]))
                .set(x.value);
        }
        HostStats::Memused(x) => {
            metrics
                .mem_used
                .get_or_create(&create_labels(&[]))
                .set(x.value);
        }
        HostStats::MemusedMax(x) => {
            metrics
                .mem_used_max
                .get_or_create(&create_labels(&[]))
                .inc_by(x.value);
        }
    }
}

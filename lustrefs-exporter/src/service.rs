// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::Family;
use lustre_collector::LustreServiceStats;
use prometheus_client::{
    metrics::{counter::Counter, gauge::Gauge},
    registry::Registry,
};
use std::{ops::Deref, sync::atomic::AtomicU64};

#[derive(Debug, Default)]
pub struct ServiceMetrics {
    ldlm_canceld_stats: Family<Counter<u64>>,
    ldlm_cbd_stats: Family<Counter<u64>>,
    // GCP-226: counter-reset detection for the ldlm service stats. These
    // services are node-global (no target), so it's one start_time per
    // service per node.
    ldlm_canceld_stats_start_time: Family<Gauge<u64, AtomicU64>>,
    ldlm_cbd_stats_start_time: Family<Gauge<u64, AtomicU64>>,
}

impl ServiceMetrics {
    pub fn register_metric(&self, registry: &mut Registry) {
        registry.register_without_auto_suffix(
            "lustre_ldlm_canceld_stats",
            "Gives information about LDLM Canceld service",
            self.ldlm_canceld_stats.clone(),
        );

        registry.register_without_auto_suffix(
            "lustre_ldlm_cbd_stats",
            "Gives information about LDLM Callback service",
            self.ldlm_cbd_stats.clone(),
        );

        registry.register_without_auto_suffix(
            "lustre_ldlm_canceld_stats_start_time",
            "Time (epoch seconds) the LDLM Canceld service stats were last reset",
            self.ldlm_canceld_stats_start_time.clone(),
        );

        registry.register_without_auto_suffix(
            "lustre_ldlm_cbd_stats_start_time",
            "Time (epoch seconds) the LDLM Callback service stats were last reset",
            self.ldlm_cbd_stats_start_time.clone(),
        );
    }
}

fn start_epoch(header: &lustre_collector::StatsHeader) -> Option<u64> {
    header
        .start_time
        .as_ref()
        .and_then(|s| s.parse::<f64>().ok())
        .map(|f| f as u64)
}

pub fn build_service_stats(x: &LustreServiceStats, service: &mut ServiceMetrics) {
    match x {
        LustreServiceStats::LdlmCanceld(header, xs) => {
            if let Some(start) = start_epoch(header) {
                service
                    .ldlm_canceld_stats_start_time
                    .get_or_create(&vec![])
                    .set(start);
            }

            for s in xs {
                service
                    .ldlm_canceld_stats
                    .get_or_create(&vec![("operation", s.name.deref().to_string())])
                    .inc_by(s.samples);
            }
        }
        LustreServiceStats::LdlmCbd(header, xs) => {
            if let Some(start) = start_epoch(header) {
                service
                    .ldlm_cbd_stats_start_time
                    .get_or_create(&vec![])
                    .set(start);
            }

            for s in xs {
                service
                    .ldlm_cbd_stats
                    .get_or_create(&vec![("operation", s.name.deref().to_string())])
                    .inc_by(s.samples);
            }
        }
    }
}

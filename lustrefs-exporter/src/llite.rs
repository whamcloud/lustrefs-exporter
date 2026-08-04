// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::Family;
use lustre_collector::LliteStat;
use prometheus_client::{
    metrics::{counter::Counter, gauge::Gauge},
    registry::Registry,
};
use std::{ops::Deref, sync::atomic::AtomicU64};

#[derive(Debug, Default)]
pub struct LliteMetrics {
    client_stats: Family<Counter<u64>>,
    client_stats_start_time: Family<Gauge<u64, AtomicU64>>,
}

impl LliteMetrics {
    pub fn register_metric(&self, registry: &mut Registry) {
        registry.register_without_auto_suffix(
            "lustre_client_stats",
            "Lustre client interface stats",
            self.client_stats.clone(),
        );

        registry.register(
            "lustre_client_stats_start_time",
            "Unix epoch seconds when lustre_client_stats was last reset",
            self.client_stats_start_time.clone(),
        );
    }
}

pub fn build_llite_stats(x: &LliteStat, metrics: &mut LliteMetrics) {
    let LliteStat {
        target,
        param: _,
        stats,
        header,
    } = x;

    // GCP-226: one start_time per target
    let start_epoch: Option<u64> = header
        .start_time
        .as_ref()
        .and_then(|s| s.parse::<f64>().ok())
        .map(|f| f as u64);

    if let Some(start) = start_epoch {
        metrics
            .client_stats_start_time
            .get_or_create(&vec![("target", target.deref().to_string())])
            .set(start);
    }

    for stat in stats {
        metrics
            .client_stats
            .get_or_create(&vec![
                ("operation", stat.name.deref().to_string()),
                ("target", target.deref().to_string()),
            ])
            .inc_by(stat.samples);
    }
}

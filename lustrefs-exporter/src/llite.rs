// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::Family;
use lustre_collector::LliteStat;
use prometheus_client::{metrics::counter::Counter, registry::Registry};
use std::ops::Deref;

#[derive(Debug, Default)]
pub struct LliteMetrics {
    client_stats: Family<Counter<u64>>,
}

impl LliteMetrics {
    pub fn register_metric(&self, registry: &mut Registry) {
        registry.register_without_auto_suffix(
            "lustre_client_stats",
            "Lustre client interface stats",
            self.client_stats.clone(),
        );
    }
}

pub fn build_llite_stats(x: &LliteStat, metrics: &mut LliteMetrics) {
    let LliteStat {
        target,
        param: _,
        stats,
    } = x;

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

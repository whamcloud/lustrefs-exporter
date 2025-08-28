// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::Family;
use lustre_collector::LustreServiceStats;
use prometheus_client::{metrics::counter::Counter, registry::Registry};
use std::ops::Deref;

#[derive(Debug, Default)]
pub struct ServiceMetrics {
    ldlm_canceld_stats: Family<Counter<u64>>,
    ldlm_cbd_stats: Family<Counter<u64>>,
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
    }
}

pub fn build_service_stats(x: &LustreServiceStats, service: &mut ServiceMetrics) {
    match x {
        LustreServiceStats::LdlmCanceld(xs) => {
            for s in xs {
                service
                    .ldlm_canceld_stats
                    .get_or_create(&vec![("operation", s.name.deref().to_string())])
                    .inc_by(s.samples);
            }
        }
        LustreServiceStats::LdlmCbd(xs) => {
            for s in xs {
                service
                    .ldlm_cbd_stats
                    .get_or_create(&vec![("operation", s.name.deref().to_string())])
                    .inc_by(s.samples);
            }
        }
    }
}

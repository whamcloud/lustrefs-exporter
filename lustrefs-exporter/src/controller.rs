// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::Family;
use lustre_collector::{ControllerState, ControllerStats};
use prometheus_client::{metrics::gauge::Gauge, registry::Registry};

#[derive(Debug, Default)]
pub struct ControllerMetrics {
    osc_state: Family<Gauge>,
}

impl ControllerMetrics {
    pub fn register_metric(&self, registry: &mut Registry) {
        registry.register(
            "lustre_osc_state",
            "Lustre OSC connection state",
            self.osc_state.clone(),
        );
    }
}

pub fn build_controller_stats(x: &ControllerStats, metrics: &mut ControllerMetrics) {
    match x {
        ControllerStats::OscState(x) => {
            let value = match x.value.current_state {
                ControllerState::Full | ControllerState::Idle => 1,
                _ => 0,
            };
            metrics
                .osc_state
                .get_or_create(&vec![
                    ("controller", x.controller.to_string()),
                    ("current_state", x.value.current_state.to_string()),
                ])
                .set(value);
        }
    }
}

// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    Family,
    client::{ClientLabels, ExtendedClientMetrics, fs_name},
    client_target,
    client_target::ClientTargetMetrics,
    histogram::HistogramEncoding,
    metrics::Metrics,
};
use lustre_collector::{ControllerState, ControllerStats, ControllerVariant};
use prometheus_client::{metrics::gauge::Gauge, registry::Registry};

#[derive(Debug)]
pub struct ControllerMetrics {
    labels: ClientLabels,
    osc_state: Family<Gauge>,
    osc: ClientTargetMetrics,
    mdc: ClientTargetMetrics,
}

impl Default for ControllerMetrics {
    fn default() -> Self {
        Self::new(
            ClientLabels::default(),
            HistogramEncoding::default(),
            ExtendedClientMetrics::default(),
        )
    }
}

impl ControllerMetrics {
    pub fn new(
        labels: ClientLabels,
        histograms: HistogramEncoding,
        extended: ExtendedClientMetrics,
    ) -> Self {
        Self {
            labels,
            osc_state: Family::default(),
            osc: ClientTargetMetrics::new(ControllerVariant::Osc, labels, histograms, extended),
            mdc: ClientTargetMetrics::new(ControllerVariant::Mdc, labels, histograms, extended),
        }
    }

    pub fn register_metric(&self, registry: &mut Registry) {
        registry.register(
            "lustre_osc_state",
            match self.labels {
                ClientLabels::PerTarget => "Lustre OSC connection state",
                ClientLabels::ByFilesystem => {
                    "Number of OSCs in each connection state, by filesystem"
                }
            },
            self.osc_state.clone(),
        );
        self.osc.register_metric(registry);
        self.mdc.register_metric(registry);
    }

    fn client_target(&mut self, kind: ControllerVariant) -> &mut ClientTargetMetrics {
        match kind {
            ControllerVariant::Osc => &mut self.osc,
            ControllerVariant::Mdc => &mut self.mdc,
        }
    }
}

/// An MDS links its OST-facing osp devices under `osc.*` (`-osc-MDT<n>`);
/// their ptlrpc `stats` must not render as client families.
fn is_osp(controller: &str) -> bool {
    controller.contains("-osc-MDT")
}

pub fn build_controller_stats(x: &ControllerStats, metrics: &mut Metrics) {
    let name = x.controller();

    if !matches!(x, ControllerStats::OscState(_)) && is_osp(name) {
        tracing::debug!("skipping osp device under osc.*: {name}");

        return;
    }

    let metrics = &mut metrics.controller;

    match x {
        ControllerStats::OscState(x) => match metrics.labels {
            ClientLabels::PerTarget => {
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
            ClientLabels::ByFilesystem => {
                metrics
                    .osc_state
                    .get_or_create(&vec![
                        ("fs", fs_name(&x.controller).to_string()),
                        ("current_state", x.value.current_state.to_string()),
                    ])
                    .inc();
            }
        },
        ControllerStats::Stats(x) => client_target::build_stats(x, metrics.client_target(x.kind)),
        ControllerStats::MdStats(x) => {
            client_target::build_md_stats(x, metrics.client_target(x.kind))
        }
        ControllerStats::RpcStats(x) => {
            client_target::build_rpc_stats(x, metrics.client_target(x.kind))
        }
        ControllerStats::IoLatencyStats(x) => {
            client_target::build_io_latency_stats(x, metrics.client_target(x.kind))
        }
        ControllerStats::LocklessStats(x) => {
            client_target::build_lockless_stats(x, metrics.client_target(x.kind))
        }
        ControllerStats::UnstableStats(x) => {
            client_target::build_unstable_stats(x, metrics.client_target(x.kind))
        }
        ControllerStats::CompressionStats(x) => {
            client_target::build_compression_stats(x, metrics.client_target(x.kind))
        }
        ControllerStats::CurGrantBytes(x) => {
            client_target::build_cur_grant_bytes(x, metrics.client_target(x.kind))
        }
        ControllerStats::CurDirtyBytes(x) => {
            client_target::build_cur_dirty_bytes(x, metrics.client_target(x.kind))
        }
    }
}

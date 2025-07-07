// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{Family, create_labels};
use lustre_collector::{LNetStat, LNetStatGlobal, LNetStats};
use prometheus_client::{metrics::counter::Counter, registry::Registry};

#[derive(Debug, Default)]
pub struct LNetMetrics {
    send_count_total: Family<Counter<u64>>,
    receive_count_total: Family<Counter<u64>>,
    drop_count_total: Family<Counter<u64>>,
    send_bytes_total: Family<Counter<u64>>,
    receive_bytes_total: Family<Counter<u64>>,
    drop_bytes_total: Family<Counter<u64>>,
}

impl LNetMetrics {
    pub fn register_metric(&self, registry: &mut Registry) {
        registry.register(
            "lustre_send_count",
            "Total number of messages that have been sent",
            self.send_count_total.clone(),
        );

        registry.register(
            "lustre_receive_count",
            "Total number of messages that have been received",
            self.receive_count_total.clone(),
        );

        registry.register(
            "lustre_drop_count",
            "Total number of messages that have been dropped",
            self.drop_count_total.clone(),
        );

        registry.register(
            "lustre_send_bytes",
            "Total number of bytes that have been sent",
            self.send_bytes_total.clone(),
        );

        registry.register(
            "lustre_receive_bytes",
            "Total number of bytes that have been received",
            self.receive_bytes_total.clone(),
        );

        registry.register(
            "lustre_drop_bytes",
            "Total number of bytes that have been dropped",
            self.drop_bytes_total.clone(),
        );
    }
}

fn record_lnet_stat(stat: &LNetStat<i64>, counter: &mut Family<Counter<u64>>) {
    let labels = create_labels(&[("nid", stat.nid.to_string())]);

    counter
        .get_or_create(&labels)
        .inc_by(stat.value.try_into().unwrap_or(0));
}

fn record_lnet_stat_global(stat: &LNetStatGlobal<i64>, counter: &mut Family<Counter<u64>>) {
    let labels = create_labels(&[]);

    counter
        .get_or_create(&labels)
        .inc_by(stat.value.try_into().unwrap_or(0));
}

pub fn build_lnet_stats(x: &LNetStats, lnet: &mut LNetMetrics) {
    match x {
        LNetStats::SendCount(stat) => {
            record_lnet_stat(stat, &mut lnet.send_count_total);
        }
        LNetStats::RecvCount(stat) => {
            record_lnet_stat(stat, &mut lnet.receive_count_total);
        }
        LNetStats::DropCount(stat) => {
            record_lnet_stat(stat, &mut lnet.drop_count_total);
        }
        LNetStats::SendLength(stat) => {
            record_lnet_stat_global(stat, &mut lnet.send_bytes_total);
        }
        LNetStats::RecvLength(stat) => {
            record_lnet_stat_global(stat, &mut lnet.receive_bytes_total);
        }
        LNetStats::DropLength(stat) => {
            record_lnet_stat_global(stat, &mut lnet.drop_bytes_total);
        }
    }
}

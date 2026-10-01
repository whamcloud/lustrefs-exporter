// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    Family,
    client::{fs_name, labels, observe_rw, set_start_time, with},
};
use lustre_collector::{
    ExtentsBucket, KeyValue, LliteStat, LliteTargetStat, ProcessExtents, RwStats,
};
use prometheus_client::{
    metrics::{counter::Counter, gauge::Gauge},
    registry::Registry,
};
use std::{ops::Deref, sync::atomic::AtomicU64};

#[derive(Debug, Default)]
pub struct LliteMetrics {
    client_stats: Family<Counter<u64>>,
    client_stats_start_time: Family<Gauge<u64, AtomicU64>>,
    read_bytes_total: Family<Counter<u64>>,
    write_bytes_total: Family<Counter<u64>>,
    read_ahead_stats_total: Family<Counter<u64>>,
    statahead_stats_total: Family<Counter<u64>>,
    extents_total: Family<Counter<u64>>,
    extents_per_process_total: Family<Counter<u64>>,
    unstable_pages: Family<Gauge<u64, AtomicU64>>,
    unstable_check: Family<Gauge<u64, AtomicU64>>,
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
        registry.register(
            "lustre_client_llite_read_bytes",
            "Total bytes read by the client",
            self.read_bytes_total.clone(),
        );
        registry.register(
            "lustre_client_llite_write_bytes",
            "Total bytes written by the client",
            self.write_bytes_total.clone(),
        );
        registry.register(
            "lustre_client_llite_read_ahead_stats",
            "Lustre read-ahead counters, by operation",
            self.read_ahead_stats_total.clone(),
        );
        registry.register(
            "lustre_client_llite_statahead_stats",
            "Lustre statahead counters, by operation",
            self.statahead_stats_total.clone(),
        );
        registry.register(
            "lustre_client_llite_extents_stats",
            "Number of I/O calls by I/O size. 'size' label is the lower bound of the size bucket in bytes",
            self.extents_total.clone(),
        );
        registry.register(
            "lustre_client_llite_extents_stats_per_process",
            "Number of I/O calls by I/O size and process. 'size' label is the lower bound of the size bucket in bytes; every 'pid' is a new series",
            self.extents_per_process_total.clone(),
        );
        registry.register(
            "lustre_client_llite_unstable_pages",
            "Number of unstable pages",
            self.unstable_pages.clone(),
        );
        registry.register(
            "lustre_client_llite_unstable_check",
            "Whether unstable page accounting is enabled",
            self.unstable_check.clone(),
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

    let target_labels = labels(target);

    set_start_time(&metrics.client_stats_start_time, &target_labels, header);

    for stat in stats {
        metrics
            .client_stats
            .get_or_create(&vec![
                ("fs", fs_name(target).to_string()),
                ("operation", stat.name.deref().to_string()),
                ("target", target.deref().to_string()),
            ])
            .inc_by(stat.samples);

        let bytes = match stat.name.as_str() {
            "read_bytes" => &metrics.read_bytes_total,
            "write_bytes" => &metrics.write_bytes_total,
            _ => continue,
        };

        if let Some(sum) = stat.sum {
            bytes.get_or_create(&target_labels).inc_by(sum);
        }
    }
}

pub fn build_read_ahead_stats(x: &LliteStat, metrics: &mut LliteMetrics) {
    let labels = labels(&x.target);

    for stat in &x.stats {
        metrics
            .read_ahead_stats_total
            .get_or_create(&with(&labels, "operation", stat.name.clone()))
            .inc_by(stat.samples);
    }
}

/// The kernel mixes `statahead total` and `hit_total`.
pub fn build_statahead_stats(x: &LliteTargetStat<Vec<KeyValue>>, metrics: &mut LliteMetrics) {
    let labels = labels(&x.target);

    for kv in &x.value {
        metrics
            .statahead_stats_total
            .get_or_create(&with(&labels, "operation", kv.name.replace(' ', "_")))
            .inc_by(kv.value);
    }
}

pub fn build_unstable_stats(x: &LliteTargetStat<Vec<KeyValue>>, metrics: &mut LliteMetrics) {
    let labels = labels(&x.target);

    for kv in &x.value {
        let family = match kv.name.as_str() {
            "unstable_pages" => &metrics.unstable_pages,
            "unstable_check" => &metrics.unstable_check,
            // unstable_mb is derived from unstable_pages.
            "unstable_mb" => continue,
            other => {
                tracing::debug!("Unhandled llite unstable_stats value: {other}");
                continue;
            }
        };

        family.get_or_create(&labels).set(kv.value);
    }
}

fn extents(buckets: &[ExtentsBucket]) -> impl Iterator<Item = (u64, u64, u64)> + '_ {
    buckets
        .iter()
        .map(|b| (b.lower_bytes, b.read_calls, b.write_calls))
}

pub fn build_extents_stats(
    x: &LliteTargetStat<RwStats<Vec<ExtentsBucket>>>,
    metrics: &mut LliteMetrics,
) {
    if let RwStats::Enabled { value, .. } = &x.value {
        observe_rw(&metrics.extents_total, &labels(&x.target), extents(value));
    }
}

pub fn build_extents_stats_per_process(
    x: &LliteTargetStat<RwStats<Vec<ProcessExtents>>>,
    metrics: &mut LliteMetrics,
) {
    if let RwStats::Enabled { value, .. } = &x.value {
        let labels = labels(&x.target);

        for process in value {
            observe_rw(
                &metrics.extents_per_process_total,
                &with(&labels, "pid", process.pid.to_string()),
                extents(&process.buckets),
            );
        }
    }
}

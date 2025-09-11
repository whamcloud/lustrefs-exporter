// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{Family, LabelProm};
use lustre_collector::{ExportStats, MdsStat, Stat, Target, TargetStat, TargetVariant};
use prometheus_client::{
    metrics::{counter::Counter, gauge::Gauge},
    registry::Registry,
};
use std::{ops::Deref, sync::atomic::AtomicU64};

#[derive(Debug, Default)]
pub struct StatsMetrics {
    // OST metrics
    read_samples_total: Family<Counter<u64>>,
    read_minimum_size_bytes: Family<Gauge<u64, AtomicU64>>,
    read_maximum_size_bytes: Family<Counter<u64>>,
    read_bytes_total: Family<Counter<u64>>,
    write_samples_total: Family<Counter<u64>>,
    write_minimum_size_bytes: Family<Gauge<u64, AtomicU64>>,
    write_maximum_size_bytes: Family<Counter<u64>>,
    write_bytes_total: Family<Counter<u64>>,

    // MDT metrics
    stats_total: Family<Counter<u64>>,
    stats_time_min: Family<Gauge<u64, AtomicU64>>,
    stats_time_max: Family<Gauge<u64, AtomicU64>>,
    stats_time_total: Family<Gauge<u64, AtomicU64>>,

    // MDS metrics
    mds_mdt_stats: Family<Gauge<u64, AtomicU64>>,
    mds_mdt_fld_stats: Family<Gauge<u64, AtomicU64>>,
    mds_mdt_io_stats: Family<Gauge<u64, AtomicU64>>,
    mds_mdt_out_stats: Family<Gauge<u64, AtomicU64>>,
    mds_mdt_readpage_stats: Family<Gauge<u64, AtomicU64>>,
    mds_mdt_seqm_stats: Family<Gauge<u64, AtomicU64>>,
    mds_mdt_seqs_stats: Family<Gauge<u64, AtomicU64>>,
    mds_mdt_setattr_stats: Family<Gauge<u64, AtomicU64>>,

    // Export metrics
    client_export_stats: Family<Counter<u64>>,
    client_export_milliseconds: Family<Counter<u64>>,
    client_export_bytes: Family<Counter<u64>>,
}

impl StatsMetrics {
    pub fn register_metric(&self, registry: &mut Registry) {
        registry.register(
            "lustre_read_samples",
            "Total number of reads that have been recorded",
            self.read_samples_total.clone(),
        );

        registry.register(
            "lustre_read_minimum_size_bytes",
            "The minimum read size in bytes",
            self.read_minimum_size_bytes.clone(),
        );

        registry.register_without_auto_suffix(
            "lustre_read_maximum_size_bytes",
            "The maximum read size in bytes",
            self.read_maximum_size_bytes.clone(),
        );

        registry.register(
            "lustre_read_bytes",
            "The total number of bytes that have been read",
            self.read_bytes_total.clone(),
        );

        registry.register(
            "lustre_write_samples",
            "Total number of writes that have been recorded",
            self.write_samples_total.clone(),
        );

        registry.register(
            "lustre_write_minimum_size_bytes",
            "The minimum write size in bytes",
            self.write_minimum_size_bytes.clone(),
        );

        registry.register_without_auto_suffix(
            "lustre_write_maximum_size_bytes",
            "The maximum write size in bytes",
            self.write_maximum_size_bytes.clone(),
        );

        registry.register(
            "lustre_write_bytes",
            "The total number of bytes that have been written",
            self.write_bytes_total.clone(),
        );

        registry.register(
            "lustre_stats",
            "Number of operations the filesystem has performed",
            self.stats_total.clone(),
        );

        registry.register(
            "lustre_stats_time_min",
            "Minimum time taken for an operation in microseconds",
            self.stats_time_min.clone(),
        );

        registry.register(
            "lustre_stats_time_max",
            "Maximum time taken for an operation in microseconds",
            self.stats_time_max.clone(),
        );

        registry.register(
            "lustre_stats_time_total",
            "Total time taken for an operation in microseconds",
            self.stats_time_total.clone(),
        );

        registry.register(
            "lustre_mds_mdt_stats",
            "MDS mdt stats",
            self.mds_mdt_stats.clone(),
        );

        registry.register(
            "lustre_mds_mdt_fld_stats",
            "MDS mdt_fld stats",
            self.mds_mdt_fld_stats.clone(),
        );

        registry.register(
            "lustre_mds_mdt_io_stats",
            "MDS mdt_io stats",
            self.mds_mdt_io_stats.clone(),
        );

        registry.register(
            "lustre_mds_mdt_out_stats",
            "MDS mdt_out stats",
            self.mds_mdt_out_stats.clone(),
        );

        registry.register(
            "lustre_mds_mdt_readpage_stats",
            "MDS mdt_readpage stats",
            self.mds_mdt_readpage_stats.clone(),
        );

        registry.register(
            "lustre_mds_mdt_seqm_stats",
            "MDS mdt_seqm stats",
            self.mds_mdt_seqm_stats.clone(),
        );

        registry.register(
            "lustre_mds_mdt_seqs_stats",
            "MDS mdt_seqs stats",
            self.mds_mdt_seqs_stats.clone(),
        );

        registry.register(
            "lustre_mds_mdt_setattr_stats",
            "MDS mdt_setattr stats",
            self.mds_mdt_setattr_stats.clone(),
        );

        registry.register_without_auto_suffix(
            "lustre_client_export_stats",
            "Number of operations the target has performed per export",
            self.client_export_stats.clone(),
        );

        registry.register(
            "lustre_client_export_milliseconds",
            "Accumulated latency per operations the target has performed per export",
            self.client_export_milliseconds.clone(),
        );

        registry.register(
            "lustre_client_export_bytes",
            "Accumulated bytes per operation the target has performed per export",
            self.client_export_bytes.clone(),
        );
    }
}

pub fn build_ost_stats(stats: &[Stat], target: &Target, metrics: &mut StatsMetrics) {
    let kind = TargetVariant::Ost;

    for s in stats {
        match s.name.as_str() {
            "read_bytes" => {
                let read_labels = vec![
                    ("component", kind.to_prom_label().to_string()),
                    ("operation", "read".into()),
                    ("target", target.deref().to_string()),
                ];

                metrics
                    .read_samples_total
                    .get_or_create(&read_labels)
                    .inc_by(s.samples);

                if let Some(min) = s.min {
                    metrics
                        .read_minimum_size_bytes
                        .get_or_create(&read_labels)
                        .set(min);
                }

                if let Some(max) = s.max {
                    metrics
                        .read_maximum_size_bytes
                        .get_or_create(&read_labels)
                        .inc_by(max);
                }

                if let Some(sum) = s.sum {
                    metrics
                        .read_bytes_total
                        .get_or_create(&read_labels)
                        .inc_by(sum);
                }
            }
            "write_bytes" => {
                let write_labels = vec![
                    ("component", kind.to_prom_label().to_string()),
                    ("operation", "write".into()),
                    ("target", target.deref().to_string()),
                ];

                metrics
                    .write_samples_total
                    .get_or_create(&write_labels)
                    .inc_by(s.samples);

                if let Some(min) = s.min {
                    metrics
                        .write_minimum_size_bytes
                        .get_or_create(&write_labels)
                        .set(min);
                }

                if let Some(max) = s.max {
                    metrics
                        .write_maximum_size_bytes
                        .get_or_create(&write_labels)
                        .inc_by(max);
                }

                if let Some(sum) = s.sum {
                    metrics
                        .write_bytes_total
                        .get_or_create(&write_labels)
                        .inc_by(sum);
                }
            }
            _ => {
                // Ignore other stats
            }
        }
    }
}

pub fn build_mdt_stats(stats: &[Stat], target: &Target, metrics: &mut StatsMetrics) {
    let kind = TargetVariant::Mdt;

    for s in stats {
        let labels = vec![
            ("component", kind.to_prom_label().to_string()),
            ("operation", s.name.deref().to_string()),
            ("target", target.deref().to_string()),
        ];

        metrics.stats_total.get_or_create(&labels).inc_by(s.samples);

        if let Some(min) = s.min {
            metrics.stats_time_min.get_or_create(&labels).inc_by(min);
        }
        if let Some(max) = s.max {
            metrics.stats_time_max.get_or_create(&labels).inc_by(max);
        }
        if let Some(sum) = s.sum {
            metrics.stats_time_total.get_or_create(&labels).inc_by(sum);
        }
    }
}

pub fn build_stats(x: &TargetStat<Vec<Stat>>, stats: &mut StatsMetrics) {
    let TargetStat {
        kind,
        target,
        value,
        ..
    } = x;

    match kind {
        TargetVariant::Ost => build_ost_stats(value, target, stats),
        TargetVariant::Mgt => { /* TODO */ }
        TargetVariant::Mdt => build_mdt_stats(value, target, stats),
    }
}

pub fn build_mds_stats(x: &MdsStat, metrics: &mut StatsMetrics) {
    let MdsStat { param, stats } = x;

    for stat in stats {
        let labels = vec![
            ("operation", stat.name.as_str().to_string()),
            ("units", stat.units.as_str().to_string()),
        ];

        match param.0.as_str() {
            "mdt" => metrics
                .mds_mdt_stats
                .get_or_create(&labels)
                .set(stat.samples),

            "mdt_fld" => metrics
                .mds_mdt_fld_stats
                .get_or_create(&labels)
                .set(stat.samples),

            "mdt_io" => metrics
                .mds_mdt_io_stats
                .get_or_create(&labels)
                .set(stat.samples),

            "mdt_out" => metrics
                .mds_mdt_out_stats
                .get_or_create(&labels)
                .set(stat.samples),

            "mdt_readpage" => metrics
                .mds_mdt_readpage_stats
                .get_or_create(&labels)
                .set(stat.samples),

            "mdt_seqm" => metrics
                .mds_mdt_seqm_stats
                .get_or_create(&labels)
                .set(stat.samples),

            "mdt_seqs" => metrics
                .mds_mdt_seqs_stats
                .get_or_create(&labels)
                .set(stat.samples),

            "mdt_setattr" => metrics
                .mds_mdt_setattr_stats
                .get_or_create(&labels)
                .set(stat.samples),

            _ => 0,
        };
    }
}

pub fn build_export_stats(x: &TargetStat<Vec<ExportStats>>, metrics: &mut StatsMetrics) {
    let TargetStat {
        kind,
        target,
        param,
        value: export_stats,
    } = x;

    if param.0.as_str() != "exports" {
        return;
    }

    for e in export_stats {
        let ExportStats { nid, stats } = e;
        for s in stats {
            let labels = vec![
                ("component", kind.to_prom_label().to_string()),
                ("name", s.name.as_str().to_string()),
                ("nid", nid.as_str().to_string()),
                ("target", target.deref().to_string()),
                ("units", s.units.as_str().to_string()),
            ];

            if let Some(sum) = s.sum {
                match s.units.as_str() {
                    "bytes" => metrics
                        .client_export_bytes
                        .get_or_create(&labels)
                        .inc_by(sum),

                    "usecs" => metrics
                        .client_export_milliseconds
                        .get_or_create(&labels)
                        .inc_by(sum),

                    _ => 0,
                };
            }

            metrics
                .client_export_stats
                .get_or_create(&labels)
                .inc_by(s.samples);
        }
    }
}

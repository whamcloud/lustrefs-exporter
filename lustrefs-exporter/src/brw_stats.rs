// Copyright (c) 2024 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use std::{
    collections::{BTreeMap, HashSet},
    ops::Deref,
};

use lustre_collector::{
    BrwStats, BrwStatsBucket, ChangeLogUser, ChangelogStat, OssStat, Stat, TargetStat, TargetStats,
};
use prometheus_exporter_base::{prelude::*, Yes};

use crate::{
    llite::build_llite_stats,
    quota::{build_ost_quota_stats, build_quota_stats},
    stats::{build_export_stats, build_mds_stats, build_stats},
    LabelProm, Metric, StatsMapExt, ToMetricInst,
};

static DISK_IO_TOTAL: Metric = Metric {
    name: "lustre_disk_io_total",
    help: "Total number of operations the filesystem has performed for the given size. 'size' label represents 'Disk I/O size', the size of each I/O operation",
    r#type: MetricType::Counter,
};

static DISK_IO_FRAGS: Metric = Metric {
    name: "lustre_dio_frags",
    help: "Current disk IO fragmentation for the given size. 'size' label represents 'Disk fragmented I/Os', the number of I/Os that were not written entirely sequentially.",
    r#type: MetricType::Gauge,
};

static DISK_IO: Metric = Metric {
    name: "lustre_disk_io",
    help: "Current number of I/O operations that are processing during the snapshot. 'size' label represents 'Disk I/Os in flight', the number of disk I/Os currently pending.",
    r#type: MetricType::Gauge,
};

static DISCONTIGUOUS_PAGES_TOTAL: Metric = Metric {
    name: "lustre_discontiguous_pages_total",
    help: "Total number of logical discontinuities per RPC. 'size' label represents 'Discontiguous pages', the number of discontinuities in the logical file offset of each page in a single RPC.",
    r#type: MetricType::Counter,
};

static DISCONTIGUOUS_BLOCKS_TOTAL: Metric = Metric {
    name: "lustre_discontiguous_blocks_total",
    help: "'size' label represents 'Discontiguous blocks', the number of discontinuities in the physical block allocation in the file system for a single RPC",
    r#type: MetricType::Counter,
};

static IO_TIME_MILLISECONDS_TOTAL: Metric = Metric {
    name: "lustre_io_time_milliseconds_total",
    help: "Total time in milliseconds the filesystem has spent processing various object sizes. 'size' label represents 'I/O time (1/1000s)', the amount of time for each I/O operation to complete.",
    r#type: MetricType::Counter,
};

static PAGES_PER_BULK_RW_TOTAL: Metric = Metric {
    name: "lustre_pages_per_bulk_rw_total",
    help: "Total number of pages per block RPC. 'size' label represents 'Pages per bulk r/w', the number of pages per RPC request",
    r#type: MetricType::Counter,
};

static INODES_FREE: Metric = Metric {
    name: "lustre_inodes_free",
    help: "The number of inodes (objects) available",
    r#type: MetricType::Gauge,
};

static INODES_MAXIMUM: Metric = Metric {
    name: "lustre_inodes_maximum",
    help: "The maximum number of inodes (objects) the filesystem can hold",
    r#type: MetricType::Gauge,
};

static AVAILABLE_KBYTES: Metric = Metric {
    name: "lustre_available_kilobytes",
    help: "Number of kilobytes readily available in the pool",
    r#type: MetricType::Gauge,
};

static FREE_KBYTES: Metric = Metric {
    name: "lustre_free_kilobytes",
    help: "Number of kilobytes allocated to the pool",
    r#type: MetricType::Gauge,
};

static CAPACITY_KBYTES: Metric = Metric {
    name: "lustre_capacity_kilobytes",
    help: "Capacity of the pool in kilobytes",
    r#type: MetricType::Gauge,
};

static EXPORTS_TOTAL: Metric = Metric {
    name: "lustre_exports_total",
    help: "Total number of times the pool has been exported",
    r#type: MetricType::Counter,
};

static EXPORTS_DIRTY_TOTAL: Metric = Metric {
    name: "lustre_exports_dirty_total",
    help: "Total number of exports that have been marked dirty",
    r#type: MetricType::Counter,
};

static EXPORTS_GRANTED_TOTAL: Metric = Metric {
    name: "lustre_exports_granted_total",
    help: "Total number of exports that have been marked granted",
    r#type: MetricType::Counter,
};

static EXPORTS_PENDING_TOTAL: Metric = Metric {
    name: "lustre_exports_pending_total",
    help: "Total number of exports that have been marked pending",
    r#type: MetricType::Counter,
};

static LOCK_CONTENDED_TOTAL: Metric = Metric {
    name: "lustre_lock_contended_total",
    help: "Number of contended locks",
    r#type: MetricType::Counter,
};

static LOCK_CONTENTION_SECONDS_TOTAL: Metric = Metric {
    name: "lustre_lock_contention_seconds_total",
    help: "Time in seconds during which locks were contended",
    r#type: MetricType::Counter,
};

static CONNECTED_CLIENTS: Metric = Metric {
    name: "lustre_connected_clients",
    help: "Number of connected clients",
    r#type: MetricType::Gauge,
};

static LOCK_COUNT_TOTAL: Metric = Metric {
    name: "lustre_lock_count_total",
    help: "Number of locks",
    r#type: MetricType::Counter,
};

static LOCK_TIMEOUT_TOTAL: Metric = Metric {
    name: "lustre_lock_timeout_total",
    help: "Number of lock timeouts",
    r#type: MetricType::Counter,
};

static BLOCK_MAPS_MSEC_TOTAL: Metric = Metric {
    name: "lustre_block_maps_milliseconds_total",
    help: "Number of block maps in milliseconds",
    r#type: MetricType::Counter,
};

static RECOVERY_STATUS: Metric = Metric {
    name: "recovery_status",
    help: "Gives the recovery status off a target. 0=Complete 1=Inactive 2=Waiting 3=WaitingForClients 4=Recovering 5=Unknown
    }",
    r#type: MetricType::Summary,
};

static RECOVERY_STATUS_COMPLETED_CLIENTS: Metric = Metric {
    name: "recovery_status_completed_clients",
    help: "Gives the count of clients that complete the recovery on a target.",
    r#type: MetricType::Gauge,
};

static RECOVERY_STATUS_CONNECTED_CLIENTS: Metric = Metric {
    name: "recovery_status_connected_clients",
    help: "Gives the count of clients connected to a target.",
    r#type: MetricType::Gauge,
};

static RECOVERY_STATUS_EVICTED_CLIENTS: Metric = Metric {
    name: "recovery_status_evicted_clients",
    help: "Gives the count of clients evicted from a target.",
    r#type: MetricType::Gauge,
};

fn build_brw_stats(
    x: TargetStat<Vec<BrwStats>>,
    stats_map: &mut BTreeMap<&'static str, PrometheusMetric<'static>>,
    set: &mut HashSet<(String, String, String, String)>,
) {
    let TargetStat {
        kind,
        target,
        value,
        ..
    } = x;

    for x in value {
        let BrwStats { name, buckets, .. } = x;

        if !buckets.is_empty() {
            let metric = match name.as_str() {
                "disk_iosize" => stats_map.get_mut_metric(DISK_IO_TOTAL),
                "rpc_hist" => stats_map.get_mut_metric(DISK_IO),
                "pages" => stats_map.get_mut_metric(PAGES_PER_BULK_RW_TOTAL),
                "discont_pages" => stats_map.get_mut_metric(DISCONTIGUOUS_PAGES_TOTAL),
                "dio_frags" => stats_map.get_mut_metric(DISK_IO_FRAGS),
                "discont_blocks" => stats_map.get_mut_metric(DISCONTIGUOUS_BLOCKS_TOTAL),
                "io_time" => stats_map.get_mut_metric(IO_TIME_MILLISECONDS_TOTAL),
                "block_maps_msec" => stats_map.get_mut_metric(BLOCK_MAPS_MSEC_TOTAL),
                _ => continue,
            };

            for b in buckets {
                let size = b.name.to_string();

                if set.insert((
                    kind.to_prom_label().to_string(),
                    target.to_string(),
                    size.clone(),
                    name.clone(),
                )) {
                    let (r, w) = rw_inst(b, kind.to_prom_label(), target.deref());

                    metric
                        .render_and_append_instance(&r.with_label("size", size.as_str()))
                        .render_and_append_instance(&w.with_label("size", size.as_str()));
                }
            }
        }
    }
}

static OST_STATS: Metric = Metric {
    name: "lustre_oss_ost_stats",
    help: "OSS ost stats",
    r#type: MetricType::Gauge,
};

static OST_IO_STATS: Metric = Metric {
    name: "lustre_oss_ost_io_stats",
    help: "OSS ost_io stats",
    r#type: MetricType::Gauge,
};

static OST_CREATE_STATS: Metric = Metric {
    name: "lustre_oss_ost_create_stats",
    help: "OSS ost_create stats",
    r#type: MetricType::Gauge,
};

static CHANGELOG_CURRENT_INDEX: Metric = Metric {
    name: "lustre_changelog_current_index",
    help: "current changelog index.",
    r#type: MetricType::Gauge,
};

static CHANGELOG_USER_INDEX: Metric = Metric {
    name: "lustre_changelog_user_index",
    help: "current, maximum changelog index per registered changelog user.",
    r#type: MetricType::Gauge,
};

static CHANGELOG_USER_IDLE_SEC: Metric = Metric {
    name: "lustre_changelog_user_idle_sec",
    help: "current changelog user idle seconds.",
    r#type: MetricType::Gauge,
};

fn build_oss_stats(x: OssStat, stats_map: &mut BTreeMap<&'static str, PrometheusMetric<'static>>) {
    let OssStat { param, stats } = x;

    for x in stats {
        let Stat {
            name,
            units,
            samples,
            ..
        } = x;

        let metric = match param.0.as_str() {
            "ost" => stats_map.get_mut_metric(OST_STATS),
            "ost_io" => stats_map.get_mut_metric(OST_IO_STATS),
            "ost_create" => stats_map.get_mut_metric(OST_CREATE_STATS),
            _ => continue,
        };

        let stat = PrometheusInstance::new()
            .with_label("operation", name.as_str())
            .with_label("units", units.as_str())
            .with_value(samples);

        metric.render_and_append_instance(&stat);
    }
}

fn build_changelog_stats(
    x: TargetStat<ChangelogStat>,
    stats_map: &mut BTreeMap<&'static str, PrometheusMetric<'static>>,
) {
    let TargetStat {
        kind: _,
        target,
        param: _,
        value,
    } = x;

    let ChangelogStat {
        current_index,
        users,
    } = value;

    for user in users {
        let ChangeLogUser {
            user,
            index,
            idle_secs,
        } = user;

        let user_index = PrometheusInstance::new()
            .with_label("user", user.as_str())
            .with_label("target", target.deref())
            .with_value(index);

        let user_idle = PrometheusInstance::new()
            .with_label("user", user.as_str())
            .with_value(idle_secs);

        stats_map
            .get_mut_metric(CHANGELOG_USER_INDEX)
            .render_and_append_instance(&user_index);
        stats_map
            .get_mut_metric(CHANGELOG_USER_IDLE_SEC)
            .render_and_append_instance(&user_idle);
    }
    let current_index = PrometheusInstance::new()
        .with_label("target", target.deref())
        .with_value(current_index);
    stats_map
        .get_mut_metric(CHANGELOG_CURRENT_INDEX)
        .render_and_append_instance(&current_index);
}

fn rw_inst<'a>(
    x: BrwStatsBucket,
    kind: &'a str,
    target: &'a str,
) -> (
    PrometheusInstance<'a, u64, Yes>,
    PrometheusInstance<'a, u64, Yes>,
) {
    let read = PrometheusInstance::new()
        .with_label("component", kind)
        .with_label("operation", "read")
        .with_label("target", target)
        .with_value(x.read);

    let write = PrometheusInstance::new()
        .with_label("component", kind)
        .with_label("operation", "write")
        .with_label("target", target)
        .with_value(x.write);

    (read, write)
}

pub fn build_target_stats(
    x: TargetStats,
    stats_map: &mut BTreeMap<&'static str, PrometheusMetric<'static>>,
    set: &mut HashSet<(String, String, String, String)>,
) {
    match x {
        TargetStats::Stats(x) => {
            build_stats(x, stats_map);
        }
        TargetStats::BrwStats(x) => {
            build_brw_stats(x, stats_map, set);
        }
        TargetStats::FilesFree(x) => {
            stats_map
                .get_mut_metric(INODES_FREE)
                .render_and_append_instance(&x.to_metric_inst());
        }
        TargetStats::FilesTotal(x) => {
            stats_map
                .get_mut_metric(INODES_MAXIMUM)
                .render_and_append_instance(&x.to_metric_inst());
        }
        TargetStats::FsType(_) => {}
        TargetStats::KBytesAvail(x) => {
            stats_map
                .get_mut_metric(AVAILABLE_KBYTES)
                .render_and_append_instance(&x.to_metric_inst());
        }
        TargetStats::KBytesFree(x) => {
            stats_map
                .get_mut_metric(FREE_KBYTES)
                .render_and_append_instance(&x.to_metric_inst());
        }
        TargetStats::KBytesTotal(x) => {
            stats_map
                .get_mut_metric(CAPACITY_KBYTES)
                .render_and_append_instance(&x.to_metric_inst());
        }
        TargetStats::NumExports(x) => {
            stats_map
                .get_mut_metric(EXPORTS_TOTAL)
                .render_and_append_instance(&x.to_metric_inst());
        }
        TargetStats::TotDirty(x) => {
            stats_map
                .get_mut_metric(EXPORTS_DIRTY_TOTAL)
                .render_and_append_instance(&x.to_metric_inst());
        }
        TargetStats::TotGranted(x) => {
            stats_map
                .get_mut_metric(EXPORTS_GRANTED_TOTAL)
                .render_and_append_instance(&x.to_metric_inst());
        }
        TargetStats::TotPending(x) => {
            stats_map
                .get_mut_metric(EXPORTS_PENDING_TOTAL)
                .render_and_append_instance(&x.to_metric_inst());
        }
        TargetStats::ContendedLocks(x) => {
            stats_map
                .get_mut_metric(LOCK_CONTENDED_TOTAL)
                .render_and_append_instance(&x.to_metric_inst());
        }
        TargetStats::ContentionSeconds(x) => {
            stats_map
                .get_mut_metric(LOCK_CONTENTION_SECONDS_TOTAL)
                .render_and_append_instance(&x.to_metric_inst());
        }
        TargetStats::ConnectedClients(x) => {
            stats_map
                .get_mut_metric(CONNECTED_CLIENTS)
                .render_and_append_instance(&x.to_metric_inst());
        }

        TargetStats::CtimeAgeLimit(_x) => {}
        TargetStats::EarlyLockCancel(_x) => {}
        TargetStats::FsNames(_x) => {}
        TargetStats::LockCount(x) => {
            stats_map
                .get_mut_metric(LOCK_COUNT_TOTAL)
                .render_and_append_instance(&x.to_metric_inst());
        }
        TargetStats::LockTimeouts(x) => {
            stats_map
                .get_mut_metric(LOCK_TIMEOUT_TOTAL)
                .render_and_append_instance(&x.to_metric_inst());
        }
        TargetStats::LockUnusedCount(_x) => {}
        TargetStats::LruMaxAge(_x) => {}
        TargetStats::LruSize(_x) => {}
        TargetStats::Llite(x) => build_llite_stats(x, stats_map),
        TargetStats::MaxNolockBytes(_x) => {}
        TargetStats::MaxParallelAst(_x) => {}
        TargetStats::ResourceCount(_x) => {}
        TargetStats::ThreadsMin(_x) => {}
        TargetStats::ThreadsMax(_x) => {}
        TargetStats::ThreadsStarted(_x) => {}
        TargetStats::RecoveryStatus(x) => {
            stats_map
                .get_mut_metric(RECOVERY_STATUS)
                .render_and_append_instance(
                    &PrometheusInstance::new()
                        .with_label("target", x.target.deref())
                        .with_label("kind", x.kind.deref())
                        .with_value(x.value as u8),
                );
        }
        TargetStats::RecoveryCompletedClients(x) => {
            stats_map
                .get_mut_metric(RECOVERY_STATUS_COMPLETED_CLIENTS)
                .render_and_append_instance(
                    &PrometheusInstance::new()
                        .with_label("target", x.target.deref())
                        .with_label("kind", x.kind.deref())
                        .with_value(x.value),
                );
        }
        TargetStats::RecoveryConnectedClients(x) => {
            stats_map
                .get_mut_metric(RECOVERY_STATUS_CONNECTED_CLIENTS)
                .render_and_append_instance(
                    &PrometheusInstance::new()
                        .with_label("target", x.target.deref())
                        .with_label("kind", x.kind.deref())
                        .with_value(x.value),
                );
        }
        TargetStats::RecoveryEvictedClients(x) => {
            stats_map
                .get_mut_metric(RECOVERY_STATUS_EVICTED_CLIENTS)
                .render_and_append_instance(
                    &PrometheusInstance::new()
                        .with_label("target", x.target.deref())
                        .with_label("kind", x.kind.deref())
                        .with_value(x.value),
                );
        }
        TargetStats::ExportStats(x) => {
            build_export_stats(x, stats_map);
        }
        TargetStats::QuotaStats(x) => {
            build_quota_stats(x, stats_map);
        }
        TargetStats::QuotaStatsOsd(x) => {
            build_ost_quota_stats(x, stats_map);
        }
        TargetStats::Oss(x) => build_oss_stats(x, stats_map),
        TargetStats::Changelog(x) => build_changelog_stats(x, stats_map),
        TargetStats::Mds(x) => build_mds_stats(x, stats_map),
    };
}

pub mod opentelemetry {
    use std::collections::HashSet;

    use lustre_collector::{
        BrwStats, ChangeLogUser, ChangelogStat, OssStat, Stat, TargetStat, TargetStats,
    };
    use opentelemetry::{
        metrics::{Counter, Gauge, Meter},
        KeyValue,
    };

    use crate::llite::opentelemetry::build_llite_stats;
    use crate::quota::opentelemetry::{build_ost_quota_stats, build_quota_stats};
    use crate::stats::opentelemetry::{build_export_stats, build_mds_stats, build_stats};
    use crate::{openmetrics::OpenTelemetryMetrics, LabelProm as _};

    #[derive(Debug)]
    pub struct OpenTelemetryMetricsBrw {
        pub disk_io_total: Counter<u64>,
        pub disk_io_frags: Gauge<u64>,
        pub disk_io: Gauge<u64>,
        pub discontiguous_pages_total: Counter<u64>,
        pub discontiguous_blocks_total: Counter<u64>,
        pub io_time_milliseconds_total: Counter<u64>,
        pub pages_per_bulk_rw_total: Counter<u64>,
        pub inodes_free: Gauge<u64>,
        pub inodes_maximum: Gauge<u64>,
        pub available_kbytes: Gauge<u64>,
        pub free_kbytes: Gauge<u64>,
        pub capacity_kbytes: Gauge<u64>,
        pub exports_total: Counter<u64>,
        pub exports_dirty_total: Counter<u64>,
        pub exports_granted_total: Counter<u64>,
        pub exports_pending_total: Counter<u64>,
        pub lock_contended_total: Counter<u64>,
        pub lock_contention_seconds_total: Counter<u64>,
        pub connected_clients: Gauge<u64>,
        pub lock_count_total: Counter<u64>,
        pub lock_timeout_total: Counter<u64>,
        pub block_maps_msec_total: Counter<u64>,
        pub recovery_status: Gauge<u64>,
        pub recovery_status_completed_clients: Gauge<u64>,
        pub recovery_status_connected_clients: Gauge<u64>,
        pub recovery_status_evicted_clients: Gauge<u64>,
        pub ost_stats: Gauge<u64>,
        pub ost_io_stats: Gauge<u64>,
        pub ost_create_stats: Gauge<u64>,
        pub changelog_current_index: Gauge<u64>,
        pub changelog_user_index: Gauge<u64>,
        pub changelog_user_idle_sec: Gauge<u64>,
    }

    impl OpenTelemetryMetricsBrw {
        pub fn new(meter: &Meter) -> Self {
            OpenTelemetryMetricsBrw {
                disk_io_total: meter
                    .u64_counter("lustre_disk_io_total")
                    .with_description("Total number of operations the filesystem has performed for the given size. 'size' label represents 'Disk I/O size', the size of each I/O operation")
                    .build(),
                disk_io_frags: meter
                    .u64_gauge("lustre_dio_frags")
                    .with_description("Current disk IO fragmentation for the given size. 'size' label represents 'Disk fragmented I/Os', the number of I/Os that were not written entirely sequentially.")
                    .build(),
                disk_io: meter
                    .u64_gauge("lustre_disk_io")
                    .with_description("Current number of I/O operations that are processing during the snapshot. 'size' label represents 'Disk I/Os in flight', the number of disk I/Os currently pending.")
                    .build(),
                discontiguous_pages_total: meter
                    .u64_counter("lustre_discontiguous_pages_total")
                    .with_description("Total number of logical discontinuities per RPC. 'size' label represents 'Discontiguous pages', the number of discontinuities in the logical file offset of each page in a single RPC.")
                    .build(),
                discontiguous_blocks_total: meter
                    .u64_counter("lustre_discontiguous_blocks_total")
                    .with_description("'size' label represents 'Discontiguous blocks', the number of discontinuities in the physical block allocation in the file system for a single RPC")
                    .build(),
                io_time_milliseconds_total: meter
                    .u64_counter("lustre_io_time_milliseconds_total")
                    .with_description("Total time in milliseconds the filesystem has spent processing various object sizes. 'size' label represents 'I/O time (1/1000s)', the amount of time for each I/O operation to complete.")
                    .build(),
                pages_per_bulk_rw_total: meter
                    .u64_counter("lustre_pages_per_bulk_rw_total")
                    .with_description("Total number of pages per block RPC. 'size' label represents 'Pages per bulk r/w', the number of pages per RPC request")
                    .build(),
                inodes_free: meter
                    .u64_gauge("lustre_inodes_free")
                    .with_description("The number of inodes (objects) available")
                    .build(),
                inodes_maximum: meter
                    .u64_gauge("lustre_inodes_maximum")
                    .with_description("The maximum number of inodes (objects) the filesystem can hold")
                    .build(),
                available_kbytes: meter
                    .u64_gauge("lustre_available_kilobytes")
                    .with_description("Number of kilobytes readily available in the pool")
                    .build(),
                free_kbytes: meter
                    .u64_gauge("lustre_free_kilobytes")
                    .with_description("Number of kilobytes allocated to the pool")
                    .build(),
                capacity_kbytes: meter
                    .u64_gauge("lustre_capacity_kilobytes")
                    .with_description("Capacity of the pool in kilobytes")
                    .build(),
                exports_total: meter
                    .u64_counter("lustre_exports_total")
                    .with_description("Total number of times the pool has been exported")
                    .build(),
                exports_dirty_total: meter
                    .u64_counter("lustre_exports_dirty_total")
                    .with_description("Total number of exports that have been marked dirty")
                    .build(),
                exports_granted_total: meter
                    .u64_counter("lustre_exports_granted_total")
                    .with_description("Total number of exports that have been marked granted")
                    .build(),
                exports_pending_total: meter
                    .u64_counter("lustre_exports_pending_total")
                    .with_description("Total number of exports that have been marked pending")
                    .build(),
                lock_contended_total: meter
                    .u64_counter("lustre_lock_contended_total")
                    .with_description("Number of contended locks")
                    .build(),
                lock_contention_seconds_total: meter
                    .u64_counter("lustre_lock_contention_seconds_total")
                    .with_description("Time in seconds during which locks were contended")
                    .build(),
                connected_clients: meter
                    .u64_gauge("lustre_connected_clients")
                    .with_description("Number of connected clients")
                    .build(),
                lock_count_total: meter
                    .u64_counter("lustre_lock_count_total")
                    .with_description("Number of locks")
                    .build(),
                lock_timeout_total: meter
                    .u64_counter("lustre_lock_timeout_total")
                    .with_description("Number of lock timeouts")
                    .build(),
                block_maps_msec_total: meter
                    .u64_counter("lustre_block_maps_milliseconds_total")
                    .with_description("Number of block maps in milliseconds")
                    .build(),
                recovery_status: meter
                    .u64_gauge("recovery_status")
                    .with_description("Gives the recovery status off a target. 0=Complete 1=Inactive 2=Waiting 3=WaitingForClients 4=Recovering 5=Unknown")
                    .build(),
                recovery_status_completed_clients: meter
                    .u64_gauge("recovery_status_completed_clients")
                    .with_description("Gives the count of clients that complete the recovery on a target")
                    .build(),
                recovery_status_connected_clients: meter
                    .u64_gauge("recovery_status_connected_clients")
                    .with_description("Gives the count of clients connected to a target")
                    .build(),
                recovery_status_evicted_clients: meter
                    .u64_gauge("recovery_status_evicted_clients")
                    .with_description("Gives the count of clients evicted from a target")
                    .build(),
                ost_stats: meter
                    .u64_gauge("lustre_oss_ost_stats")
                    .with_description("OSS ost stats")
                    .build(),
                ost_io_stats: meter
                    .u64_gauge("lustre_oss_ost_io_stats")
                    .with_description("OSS ost_io stats")
                    .build(),
                ost_create_stats: meter
                    .u64_gauge("lustre_oss_ost_create_stats")
                    .with_description("OSS ost_create stats")
                    .build(),
                changelog_current_index: meter
                    .u64_gauge("lustre_changelog_current_index")
                    .with_description("current changelog index.")
                    .build(),
                changelog_user_index: meter
                    .u64_gauge("lustre_changelog_user_index")
                    .with_description("current, maximum changelog index per registered changelog user.")
                    .build(),
                changelog_user_idle_sec: meter
                    .u64_gauge("lustre_changelog_user_idle_sec")
                    .with_description("current changelog user idle seconds.")
                    .build(),
            }
        }
    }

    fn build_brw_stats(
        x: &TargetStat<Vec<BrwStats>>,
        otel_brw: &OpenTelemetryMetricsBrw,
        set: &mut HashSet<(String, String, String, String)>,
    ) {
        let TargetStat {
            kind,
            target,
            value,
            ..
        } = x;

        for x in value {
            let BrwStats { name, buckets, .. } = x;

            for b in buckets {
                let size = b.name.to_string();
                let labels = &[
                    KeyValue::new("component", kind.to_prom_label().to_string()),
                    KeyValue::new("target", target.to_string()),
                    KeyValue::new("size", size.clone()),
                    KeyValue::new("operation", "read"),
                ];
                let write_labels = &[
                    KeyValue::new("component", kind.to_prom_label().to_string()),
                    KeyValue::new("target", target.to_string()),
                    KeyValue::new("size", size.clone()),
                    KeyValue::new("operation", "write"),
                ];

                if set.insert((
                    kind.to_prom_label().to_string(),
                    target.to_string(),
                    size.clone(),
                    name.clone(),
                )) {
                    match name.as_str() {
                        "disk_iosize" => {
                            otel_brw.disk_io_total.add(b.read, labels);
                            otel_brw.disk_io_total.add(b.write, write_labels);
                        }
                        "rpc_hist" => {
                            otel_brw.disk_io.record(b.read, labels);
                            otel_brw.disk_io.record(b.write, write_labels);
                        }
                        "pages" => {
                            otel_brw.pages_per_bulk_rw_total.add(b.read, labels);
                            otel_brw.pages_per_bulk_rw_total.add(b.write, write_labels);
                        }
                        "discont_pages" => {
                            otel_brw.discontiguous_pages_total.add(b.read, labels);
                            otel_brw
                                .discontiguous_pages_total
                                .add(b.write, write_labels);
                        }
                        "dio_frags" => {
                            otel_brw.disk_io_frags.record(b.read, labels);
                            otel_brw.disk_io_frags.record(b.write, write_labels);
                        }
                        "discont_blocks" => {
                            otel_brw.discontiguous_blocks_total.add(b.read, labels);
                            otel_brw
                                .discontiguous_blocks_total
                                .add(b.write, write_labels);
                        }
                        "io_time" => {
                            otel_brw.io_time_milliseconds_total.add(b.read, labels);
                            otel_brw
                                .io_time_milliseconds_total
                                .add(b.write, write_labels);
                        }
                        "block_maps_msec" => {
                            otel_brw.block_maps_msec_total.add(b.read, labels);
                            otel_brw.block_maps_msec_total.add(b.write, write_labels);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn build_oss_stats(x: &OssStat, otel_brw: &OpenTelemetryMetricsBrw) {
        let OssStat { param, stats } = x;

        for x in stats {
            let Stat {
                name,
                units,
                samples,
                ..
            } = x;

            let labels = &[
                KeyValue::new("operation", name.to_string()),
                KeyValue::new("units", units.to_string()),
            ];

            match param.0.as_str() {
                "ost" => otel_brw.ost_stats.record(*samples, labels),
                "ost_io" => otel_brw.ost_io_stats.record(*samples, labels),
                "ost_create" => otel_brw.ost_create_stats.record(*samples, labels),
                _ => {}
            }
        }
    }

    fn build_changelog_stats(x: &TargetStat<ChangelogStat>, otel_brw: &OpenTelemetryMetricsBrw) {
        let TargetStat { target, value, .. } = x;

        let ChangelogStat {
            current_index,
            users,
        } = value;

        otel_brw.changelog_current_index.record(
            *current_index,
            &[KeyValue::new("target", target.to_string())],
        );

        for user in users {
            let ChangeLogUser {
                user,
                index,
                idle_secs,
            } = user;

            otel_brw.changelog_user_index.record(
                *index,
                &[
                    KeyValue::new("user", user.to_string()),
                    KeyValue::new("target", target.to_string()),
                ],
            );

            otel_brw
                .changelog_user_idle_sec
                .record(*idle_secs, &[KeyValue::new("user", user.to_string())]);
        }
    }

    pub fn build_target_stats(
        x: &TargetStats,
        otel: &OpenTelemetryMetrics,
        set: &mut HashSet<(String, String, String, String)>,
    ) {
        match x {
            TargetStats::Stats(x) => {
                build_stats(x, &otel.stats);
            }
            TargetStats::BrwStats(x) => {
                build_brw_stats(x, &otel.brw, set);
            }
            TargetStats::FilesFree(x) => {
                otel.brw.inodes_free.record(
                    x.value,
                    &[
                        KeyValue::new("component", x.kind.to_prom_label().to_string()),
                        KeyValue::new("target", x.target.to_string()),
                    ],
                );
            }
            TargetStats::FilesTotal(x) => {
                otel.brw.inodes_maximum.record(
                    x.value,
                    &[
                        KeyValue::new("component", x.kind.to_prom_label().to_string()),
                        KeyValue::new("target", x.target.to_string()),
                    ],
                );
            }
            TargetStats::KBytesAvail(x) => {
                otel.brw.available_kbytes.record(
                    x.value,
                    &[
                        KeyValue::new("component", x.kind.to_prom_label().to_string()),
                        KeyValue::new("target", x.target.to_string()),
                    ],
                );
            }
            TargetStats::KBytesFree(x) => {
                otel.brw.free_kbytes.record(
                    x.value,
                    &[
                        KeyValue::new("component", x.kind.to_prom_label().to_string()),
                        KeyValue::new("target", x.target.to_string()),
                    ],
                );
            }
            TargetStats::KBytesTotal(x) => {
                otel.brw.capacity_kbytes.record(
                    x.value,
                    &[
                        KeyValue::new("component", x.kind.to_prom_label().to_string()),
                        KeyValue::new("target", x.target.to_string()),
                    ],
                );
            }
            TargetStats::NumExports(x) => {
                otel.brw.exports_total.add(
                    x.value,
                    &[
                        KeyValue::new("component", x.kind.to_prom_label().to_string()),
                        KeyValue::new("target", x.target.to_string()),
                    ],
                );
            }
            TargetStats::TotDirty(x) => {
                otel.brw.exports_dirty_total.add(
                    x.value,
                    &[
                        KeyValue::new("component", x.kind.to_prom_label().to_string()),
                        KeyValue::new("target", x.target.to_string()),
                    ],
                );
            }
            TargetStats::TotGranted(x) => {
                otel.brw.exports_granted_total.add(
                    x.value,
                    &[
                        KeyValue::new("component", x.kind.to_prom_label().to_string()),
                        KeyValue::new("target", x.target.to_string()),
                    ],
                );
            }
            TargetStats::TotPending(x) => {
                otel.brw.exports_pending_total.add(
                    x.value,
                    &[
                        KeyValue::new("component", x.kind.to_prom_label().to_string()),
                        KeyValue::new("target", x.target.to_string()),
                    ],
                );
            }
            TargetStats::ContendedLocks(x) => {
                otel.brw.lock_contended_total.add(
                    x.value,
                    &[
                        KeyValue::new("component", x.kind.to_prom_label().to_string()),
                        KeyValue::new("target", x.target.to_string()),
                    ],
                );
            }
            TargetStats::ContentionSeconds(x) => {
                otel.brw.lock_contention_seconds_total.add(
                    x.value,
                    &[
                        KeyValue::new("component", x.kind.to_prom_label().to_string()),
                        KeyValue::new("target", x.target.to_string()),
                    ],
                );
            }
            TargetStats::ConnectedClients(x) => {
                otel.brw.connected_clients.record(
                    x.value,
                    &[
                        KeyValue::new("component", x.kind.to_prom_label().to_string()),
                        KeyValue::new("target", x.target.to_string()),
                    ],
                );
            }
            TargetStats::LockCount(x) => {
                otel.brw.lock_count_total.add(
                    x.value,
                    &[
                        KeyValue::new("component", x.kind.to_prom_label().to_string()),
                        KeyValue::new("target", x.target.to_string()),
                    ],
                );
            }
            TargetStats::LockTimeouts(x) => {
                otel.brw.lock_timeout_total.add(
                    x.value,
                    &[
                        KeyValue::new("component", x.kind.to_prom_label().to_string()),
                        KeyValue::new("target", x.target.to_string()),
                    ],
                );
            }
            TargetStats::Llite(x) => build_llite_stats(x, &otel.llite),
            TargetStats::RecoveryStatus(x) => {
                otel.brw.recovery_status.record(
                    x.value as u64,
                    &[
                        KeyValue::new("target", x.target.to_string()),
                        KeyValue::new("kind", x.kind.to_prom_label().to_string()),
                    ],
                );
            }
            TargetStats::RecoveryCompletedClients(x) => {
                otel.brw.recovery_status_completed_clients.record(
                    x.value,
                    &[
                        KeyValue::new("target", x.target.to_string()),
                        KeyValue::new("kind", x.kind.to_prom_label().to_string()),
                    ],
                );
            }
            TargetStats::RecoveryConnectedClients(x) => {
                otel.brw.recovery_status_connected_clients.record(
                    x.value,
                    &[
                        KeyValue::new("target", x.target.to_string()),
                        KeyValue::new("kind", x.kind.to_prom_label().to_string()),
                    ],
                );
            }
            TargetStats::RecoveryEvictedClients(x) => {
                otel.brw.recovery_status_evicted_clients.record(
                    x.value,
                    &[
                        KeyValue::new("target", x.target.to_string()),
                        KeyValue::new("kind", x.kind.to_prom_label().to_string()),
                    ],
                );
            }
            TargetStats::ExportStats(x) => {
                build_export_stats(x, &otel.export);
            }
            TargetStats::QuotaStats(x) => {
                build_quota_stats(x, &otel.quota);
            }
            TargetStats::QuotaStatsOsd(x) => {
                build_ost_quota_stats(x, &otel.quota);
            }
            TargetStats::Oss(x) => build_oss_stats(x, &otel.brw),
            TargetStats::Changelog(x) => build_changelog_stats(x, &otel.brw),
            TargetStats::Mds(x) => build_mds_stats(x, &otel.mds),
            _ => {}
        };
    }
}

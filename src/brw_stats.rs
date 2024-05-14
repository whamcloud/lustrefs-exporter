// Copyright (c) 2024 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use std::{collections::BTreeMap, ops::Deref};

use lustre_collector::{
    BrwStats, BrwStatsBucket, ChangeLogUser, ChangelogStat, OssStat, Stat, TargetStat, TargetStats,
};
use prometheus_exporter_base::{prelude::*, Yes};

use crate::{
    jobstats::{build_mdt_job_stats, build_ost_job_stats},
    llite::build_llite_stats,
    quota::{build_ost_quota_stats, build_quota_stats},
    stats::{build_mds_stats, build_stats},
    LabelProm, Metric, StatsMapExt, ToMetricInst,
};

static DISK_IO_TOTAL: Metric = Metric {
    name: "lustre_disk_io_total",
    help: "Total number of operations the filesystem has performed for the given size.",
    r#type: MetricType::Counter,
};

static DISK_IO_FRAGS: Metric = Metric {
    name: "lustre_dio_frags",
    help: "Current disk IO fragmentation for the given size.",
    r#type: MetricType::Gauge,
};

static DISK_IO: Metric = Metric {
    name: "lustre_disk_io",
    help: "Current number of I/O operations that are processing during the snapshot.",
    r#type: MetricType::Gauge,
};

static DISCONTIGUOUS_PAGES_TOTAL: Metric = Metric {
    name: "lustre_discontiguous_pages_total",
    help: "Total number of logical discontinuities per RPC.",
    r#type: MetricType::Counter,
};

static DISCONTIGUOUS_BLOCKS_TOTAL: Metric = Metric {
    name: "lustre_discontiguous_blocks_total",
    help: "",
    r#type: MetricType::Counter,
};

static IO_TIME_MILLISECONDS_TOTAL: Metric = Metric {
    name: "lustre_io_time_milliseconds_total",
    help: "Total time in milliseconds the filesystem has spent processing various object sizes.",
    r#type: MetricType::Counter,
};

static PAGES_PER_BULK_RW_TOTAL: Metric = Metric {
    name: "lustre_pages_per_bulk_rw_total",
    help: "Total number of pages per block RPC.",
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

fn build_brw_stats(
    x: TargetStat<Vec<BrwStats>>,
    stats_map: &mut BTreeMap<&'static str, PrometheusMetric<'static>>,
) {
    let TargetStat {
        kind,
        target,
        value,
        ..
    } = x;

    for x in value {
        let BrwStats { name, buckets, .. } = x;

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

            let (r, w) = rw_inst(b, kind.to_prom_label(), target.deref());

            metric
                .render_and_append_instance(&r.with_label("size", size.as_str()))
                .render_and_append_instance(&w.with_label("size", size.as_str()));
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
) {
    match x {
        TargetStats::JobStatsOst(x) => {
            build_ost_job_stats(x, stats_map);
        }
        TargetStats::Stats(x) => {
            build_stats(x, stats_map);
        }
        TargetStats::BrwStats(x) => {
            build_brw_stats(x, stats_map);
        }
        TargetStats::JobStatsMdt(x) => {
            build_mdt_job_stats(x, stats_map);
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
        TargetStats::RecoveryStatus(_x) => {}
        TargetStats::RecoveryCompletedClients(_) => {}
        TargetStats::RecoveryConnectedClients(_) => {}
        TargetStats::RecoveryEvictedClients(_) => {}
        TargetStats::ExportStats(_) => {}
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

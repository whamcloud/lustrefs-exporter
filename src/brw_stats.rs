use std::{collections::BTreeMap, ops::Deref, time::Duration};

use lustre_collector::{BrwStats, BrwStatsBucket, TargetStat, TargetStats};
use prometheus_exporter_base::{prelude::*, Yes};

use crate::{jobstats::build_ost_job_stats, Metric, StatsMapExt, ToMetricInst};

static DISK_IO_TOTAL: Metric = Metric {
    name: "disk_io_total",
    help: "Total number of operations the filesystem has performed for the given size.",
    r#type: MetricType::Counter,
};

static DISK_IO_FRAGS: Metric = Metric {
    name: "dio_frags",
    help: "Current disk IO fragmentation for the given size.",
    r#type: MetricType::Gauge,
};

static DISK_IO: Metric = Metric {
    name: "disk_io",
    help: "Current number of I/O operations that are processing during the snapshot.",
    r#type: MetricType::Gauge,
};

static DISCONTIGUOUS_PAGES_TOTAL: Metric = Metric {
    name: "discontiguous_pages_total",
    help: "Total number of logical discontinuities per RPC.",
    r#type: MetricType::Counter,
};

static DISCONTIGUOUS_BLOCKS_TOTAL: Metric = Metric {
    name: "discontiguous_blocks_total",
    help: "",
    r#type: MetricType::Counter,
};

static IO_TIME_MILLISECONDS_TOTAL: Metric = Metric {
    name: "io_time_milliseconds_total",
    help: "Total time in milliseconds the filesystem has spent processing various object sizes.",
    r#type: MetricType::Counter,
};

static PAGES_PER_BULK_RW_TOTAL: Metric = Metric {
    name: "pages_per_bulk_rw_total",
    help: "Total number of pages per block RPC.",
    r#type: MetricType::Counter,
};

static INODES_FREE: Metric = Metric {
    name: "inodes_free",
    help: "The number of inodes (objects) available",
    r#type: MetricType::Gauge,
};

static INODES_MAXIMUM: Metric = Metric {
    name: "inodes_maximum",
    help: "The maximum number of inodes (objects) the filesystem can hold",
    r#type: MetricType::Gauge,
};

static AVAILABLE_BYTES: Metric = Metric {
    name: "available_bytes",
    help: "Number of bytes readily available in the pool",
    r#type: MetricType::Gauge,
};

static FREE_BYTES: Metric = Metric {
    name: "free_bytes",
    help: "Number of bytes allocated to the pool",
    r#type: MetricType::Gauge,
};

static CAPACITY_BYTES: Metric = Metric {
    name: "capacity_bytes",
    help: "Capacity of the pool in bytes",
    r#type: MetricType::Gauge,
};

static EXPORTS_TOTAL: Metric = Metric {
    name: "exports_total",
    help: "Total number of times the pool has been exported",
    r#type: MetricType::Counter,
};

static EXPORTS_DIRTY_TOTAL: Metric = Metric {
    name: "exports_dirty_total",
    help: "Total number of exports that have been marked dirty",
    r#type: MetricType::Counter,
};

static EXPORTS_GRANTED_TOTAL: Metric = Metric {
    name: "exports_granted_total",
    help: "Total number of exports that have been marked granted",
    r#type: MetricType::Counter,
};

static EXPORTS_PENDING_TOTAL: Metric = Metric {
    name: "exports_pending_total",
    help: "Total number of exports that have been marked pending",
    r#type: MetricType::Counter,
};

static LOCK_CONTENDED_TOTAL: Metric = Metric {
    name: "lock_contended_total",
    help: "Number of contended locks",
    r#type: MetricType::Counter,
};

static LOCK_CONTENTION_SECONDS_TOTAL: Metric = Metric {
    name: "lock_contention_seconds_total",
    help: "Time in seconds during which locks were contended",
    r#type: MetricType::Counter,
};

static CONNECTED_CLIENTS: Metric = Metric {
    name: "connected_clients",
    help: "Number of connected clients",
    r#type: MetricType::Gauge,
};

static LOCK_COUNT_TOTAL: Metric = Metric {
    name: "lock_count_total",
    help: "Number of locks",
    r#type: MetricType::Counter,
};

static LOCK_TIMEOUT_TOTAL: Metric = Metric {
    name: "lock_timeout_total",
    help: "Number of lock timeouts",
    r#type: MetricType::Counter,
};

fn build_brw_stats(
    x: TargetStat<Vec<BrwStats>>,
    stats_map: &mut BTreeMap<&'static str, PrometheusMetric<'static>>,
    time: Duration,
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
            _ => return,
        };

        for b in buckets {
            let size = b.name.to_string();

            let (r, w) = rw_inst(b, kind.deref(), target.deref(), time);

            metric
                .render_and_append_instance(&r.with_label("size", size.as_str()))
                .render_and_append_instance(&w.with_label("size", size.as_str()));
        }
    }
}

fn rw_inst<'a>(
    x: BrwStatsBucket,
    kind: &'a str,
    target: &'a str,
    time: Duration,
) -> (
    PrometheusInstance<'a, u64, Yes>,
    PrometheusInstance<'a, u64, Yes>,
) {
    let read = PrometheusInstance::new()
        .with_label("component", kind)
        .with_label("operation", "read")
        .with_label("target", target)
        .with_value(x.read)
        .with_timestamp(time.as_millis());

    let write = PrometheusInstance::new()
        .with_label("component", kind)
        .with_label("operation", "write")
        .with_label("target", target)
        .with_value(x.write)
        .with_timestamp(time.as_millis());

    (read, write)
}

pub fn build_target_stats(
    x: TargetStats,
    stats_map: &mut BTreeMap<&'static str, PrometheusMetric<'static>>,
    time: Duration,
) {
    match x {
        TargetStats::JobStatsOst(x) => {
            build_ost_job_stats(x, stats_map, time);
        }
        TargetStats::Stats(_x) => {}
        TargetStats::BrwStats(x) => {
            build_brw_stats(x, stats_map, time);
        }
        TargetStats::JobStatsMdt(_x) => {}
        TargetStats::FilesFree(x) => {
            stats_map
                .get_mut_metric(INODES_FREE)
                .render_and_append_instance(&x.to_metric_inst(time));
        }
        TargetStats::FilesTotal(x) => {
            stats_map
                .get_mut_metric(INODES_MAXIMUM)
                .render_and_append_instance(&x.to_metric_inst(time));
        }
        TargetStats::FsType(_) => {}
        TargetStats::BytesAvail(x) => {
            stats_map
                .get_mut_metric(AVAILABLE_BYTES)
                .render_and_append_instance(&x.to_metric_inst(time));
        }
        TargetStats::BytesFree(x) => {
            stats_map
                .get_mut_metric(FREE_BYTES)
                .render_and_append_instance(&x.to_metric_inst(time));
        }
        TargetStats::BytesTotal(x) => {
            stats_map
                .get_mut_metric(CAPACITY_BYTES)
                .render_and_append_instance(&x.to_metric_inst(time));
        }
        TargetStats::NumExports(x) => {
            stats_map
                .get_mut_metric(EXPORTS_TOTAL)
                .render_and_append_instance(&x.to_metric_inst(time));
        }
        TargetStats::TotDirty(x) => {
            stats_map
                .get_mut_metric(EXPORTS_DIRTY_TOTAL)
                .render_and_append_instance(&x.to_metric_inst(time));
        }
        TargetStats::TotGranted(x) => {
            stats_map
                .get_mut_metric(EXPORTS_GRANTED_TOTAL)
                .render_and_append_instance(&x.to_metric_inst(time));
        }
        TargetStats::TotPending(x) => {
            stats_map
                .get_mut_metric(EXPORTS_PENDING_TOTAL)
                .render_and_append_instance(&x.to_metric_inst(time));
        }
        TargetStats::ContendedLocks(x) => {
            stats_map
                .get_mut_metric(LOCK_CONTENDED_TOTAL)
                .render_and_append_instance(&x.to_metric_inst(time));
        }
        TargetStats::ContentionSeconds(x) => {
            stats_map
                .get_mut_metric(LOCK_CONTENTION_SECONDS_TOTAL)
                .render_and_append_instance(&x.to_metric_inst(time));
        }
        TargetStats::ConnectedClients(x) => {
            stats_map
                .get_mut_metric(CONNECTED_CLIENTS)
                .render_and_append_instance(&x.to_metric_inst(time));
        }

        TargetStats::CtimeAgeLimit(_x) => {}
        TargetStats::EarlyLockCancel(_x) => {}
        TargetStats::FsNames(_x) => {}
        TargetStats::LockCount(x) => {
            stats_map
                .get_mut_metric(LOCK_COUNT_TOTAL)
                .render_and_append_instance(&x.to_metric_inst(time));
        }
        TargetStats::LockTimeouts(x) => {
            stats_map
                .get_mut_metric(LOCK_TIMEOUT_TOTAL)
                .render_and_append_instance(&x.to_metric_inst(time));
        }
        TargetStats::LockUnusedCount(_x) => {}
        TargetStats::LruMaxAge(_x) => {}
        TargetStats::LruSize(_x) => {}
        TargetStats::MaxNolockBytes(_x) => {}
        TargetStats::MaxParallelAst(_x) => {}
        TargetStats::ResourceCount(_x) => {}
        TargetStats::ThreadsMin(_x) => {}
        TargetStats::ThreadsMax(_x) => {}
        TargetStats::ThreadsStarted(_x) => {}
        TargetStats::RecoveryStatus(_x) => {}
    };
}

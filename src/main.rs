// Copyright (c) 2022 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use lustre_collector::{
    parse_lctl_output, parser, BrwStats, BrwStatsBucket, JobStatOst, LNetStat, LNetStats, Record,
    Stat, TargetStat, TargetStats,
};
use num_traits::Num;
use prometheus_exporter_base::{prelude::*, Yes};
use std::{
    collections::BTreeMap,
    fmt,
    ops::Deref,
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use tokio::process::Command;

#[derive(Debug, Clone, Copy)]
struct Metric {
    name: &'static str,
    help: &'static str,
    r#type: MetricType,
}

impl From<Metric> for PrometheusMetric<'_> {
    fn from(x: Metric) -> Self {
        PrometheusMetric::build()
            .with_name(x.name)
            .with_help(x.help)
            .with_metric_type(x.r#type)
            .build()
    }
}

struct TargetStatsMetric {}

impl TargetStatsMetric {
    const DISK_IO_TOTAL: Metric = Metric {
        name: "disk_io_total",
        help: "Total number of operations the filesystem has performed for the given size.",
        r#type: MetricType::Counter,
    };

    const DISK_IO_FRAGS: Metric = Metric {
        name: "dio_frags",
        help: "Current disk IO fragmentation for the given size.",
        r#type: MetricType::Gauge,
    };

    const DISK_IO: Metric = Metric {
        name: "disk_io",
        help: "Current number of I/O operations that are processing during the snapshot.",
        r#type: MetricType::Gauge,
    };

    const DISCONTIGUOUS_PAGES_TOTAL: Metric = Metric {
        name: "discontiguous_pages_total",
        help: "Total number of logical discontinuities per RPC.",
        r#type: MetricType::Counter,
    };

    const DISCONTIGUOUS_BLOCKS_TOTAL: Metric = Metric {
        name: "discontiguous_blocks_total",
        help: "",
        r#type: MetricType::Counter,
    };

    const IO_TIME_MILLISECONDS_TOTAL: Metric = Metric {
        name: "io_time_milliseconds_total",
        help:
            "Total time in milliseconds the filesystem has spent processing various object sizes.",
        r#type: MetricType::Counter,
    };

    const PAGES_PER_BULK_RW_TOTAL: Metric = Metric {
        name: "pages_per_bulk_rw_total",
        help: "Total number of pages per block RPC.",
        r#type: MetricType::Counter,
    };

    const INODES_FREE: Metric = Metric {
        name: "inodes_free",
        help: "The number of inodes (objects) available",
        r#type: MetricType::Gauge,
    };

    const INODES_MAXIMUM: Metric = Metric {
        name: "inodes_maximum",
        help: "The maximum number of inodes (objects) the filesystem can hold",
        r#type: MetricType::Gauge,
    };

    const AVAILABLE_BYTES: Metric = Metric {
        name: "available_bytes",
        help: "Number of bytes readily available in the pool",
        r#type: MetricType::Gauge,
    };

    const FREE_BYTES: Metric = Metric {
        name: "free_bytes",
        help: "Number of bytes allocated to the pool",
        r#type: MetricType::Gauge,
    };

    const CAPACITY_BYTES: Metric = Metric {
        name: "capacity_bytes",
        help: "Capacity of the pool in bytes",
        r#type: MetricType::Gauge,
    };

    const EXPORTS_TOTAL: Metric = Metric {
        name: "exports_total",
        help: "Total number of times the pool has been exported",
        r#type: MetricType::Counter,
    };

    const EXPORTS_DIRTY_TOTAL: Metric = Metric {
        name: "exports_dirty_total",
        help: "Total number of exports that have been marked dirty",
        r#type: MetricType::Counter,
    };

    const EXPORTS_GRANTED_TOTAL: Metric = Metric {
        name: "exports_granted_total",
        help: "Total number of exports that have been marked granted",
        r#type: MetricType::Counter,
    };

    const EXPORTS_PENDING_TOTAL: Metric = Metric {
        name: "exports_pending_total",
        help: "Total number of exports that have been marked pending",
        r#type: MetricType::Counter,
    };

    const LOCK_CONTENDED_TOTAL: Metric = Metric {
        name: "lock_contended_total",
        help: "Number of contended locks",
        r#type: MetricType::Counter,
    };

    const LOCK_CONTENTION_SECONDS_TOTAL: Metric = Metric {
        name: "lock_contention_seconds_total",
        help: "Time in seconds during which locks were contended",
        r#type: MetricType::Counter,
    };

    const CONNECTED_CLIENTS: Metric = Metric {
        name: "connected_clients",
        help: "Number of connected clients",
        r#type: MetricType::Gauge,
    };

    const LOCK_COUNT_TOTAL: Metric = Metric {
        name: "lock_count_total",
        help: "Number of locks",
        r#type: MetricType::Counter,
    };

    const LOCK_TIMEOUT_TOTAL: Metric = Metric {
        name: "lock_timeout_total",
        help: "Number of lock timeouts",
        r#type: MetricType::Counter,
    };
}

struct LNetStatsMetric {}

impl LNetStatsMetric {
    const SEND_COUNT: Metric = Metric {
        name: "send_count_total",
        help: "Total number of messages that have been sent",
        r#type: MetricType::Counter,
    };
    const RECEIVE_COUNT: Metric = Metric {
        name: "receive_count_total",
        help: "Total number of messages that have been received",
        r#type: MetricType::Counter,
    };
    const DROP_COUNT: Metric = Metric {
        name: "drop_count_total",
        help: "Total number of messages that have been dropped",
        r#type: MetricType::Counter,
    };
}

// {"job_stats", "job_read_samples_total", readSamplesHelp, s.counterMetric, false, core},
// {"job_stats", "job_read_minimum_size_bytes", readMinimumHelp, s.gaugeMetric, false, core},
// {"job_stats", "job_read_maximum_size_bytes", readMaximumHelp, s.gaugeMetric, false, core},
// {"job_stats", "job_read_bytes_total", readTotalHelp, s.counterMetric, false, core},
// {"job_stats", "job_write_samples_total", writeSamplesHelp, s.counterMetric, false, core},
// {"job_stats", "job_write_minimum_size_bytes", writeMinimumHelp, s.gaugeMetric, false, extended},
// {"job_stats", "job_write_maximum_size_bytes", writeMaximumHelp, s.gaugeMetric, false, extended},
// {"job_stats", "job_write_bytes_total", writeTotalHelp, s.counterMetric, false, core},
// {"job_stats", "job_stats_total", jobStatsHelp, s.counterMetric, true, core},
struct JobStatOstMetric {}

impl JobStatOstMetric {
    const READ_SAMPLES: Metric = Metric {
        name: "job_read_samples_total",
        help: "Total number of reads that have been recorded.",
        r#type: MetricType::Counter,
    };
    const READ_MIN_SIZE_BYTES: Metric = Metric {
        name: "job_read_minimum_size_bytes",
        help: "The minimum read size in bytes.",
        r#type: MetricType::Gauge,
    };
    const READ_MAX_SIZE_BYTES: Metric = Metric {
        name: "job_read_maximum_size_bytes",
        help: "The maximum read size in bytes.",
        r#type: MetricType::Gauge,
    };
    const READ_BYTES: Metric = Metric {
        name: "job_read_bytes_total",
        help: "The total number of bytes that have been read.",
        r#type: MetricType::Counter,
    };

    const WRITE_SAMPLES: Metric = Metric {
        name: "job_write_samples_total",
        help: "Total number of writes that have been recorded.",
        r#type: MetricType::Counter,
    };
    const WRITE_MIN_SIZE_BYTES: Metric = Metric {
        name: "job_write_minimum_size_bytes",
        help: "The minimum write size in bytes.",
        r#type: MetricType::Gauge,
    };
    const WRITE_MAX_SIZE_BYTES: Metric = Metric {
        name: "job_write_maximum_size_bytes",
        help: "The maximum write size in bytes.",
        r#type: MetricType::Gauge,
    };
    const WRITE_BYTES: Metric = Metric {
        name: "job_write_bytes_total",
        help: "The total number of bytes that have been written.",
        r#type: MetricType::Counter,
    };
}

trait Name {
    fn name(&self) -> &'static str;
}

impl Name for Metric {
    fn name(&self) -> &'static str {
        self.name
    }
}

trait StatsMapExt {
    fn get_mut_metric<T: Name + Into<PrometheusMetric<'static>>>(
        &mut self,
        x: T,
    ) -> &mut PrometheusMetric<'static>;
}

impl StatsMapExt for BTreeMap<&'static str, PrometheusMetric<'static>> {
    fn get_mut_metric<T: Name + Into<PrometheusMetric<'static>>>(
        &mut self,
        x: T,
    ) -> &mut PrometheusMetric<'static> {
        self.entry(x.name()).or_insert_with(|| x.into())
    }
}

fn build_lustre_stats(output: Vec<Record>, time: Duration) -> String {
    let mut stats_map = BTreeMap::new();

    for x in output {
        match x {
            lustre_collector::Record::Host(_) => {}
            lustre_collector::Record::Node(_) => {}
            lustre_collector::Record::LNetStat(x) => {
                build_lnet_stats(x, &mut stats_map, time);
            }
            lustre_collector::Record::Target(x) => {
                build_target_stats(x, &mut stats_map, time);
            }
        }
    }

    stats_map
        .values()
        .map(|x| x.render())
        .collect::<Vec<_>>()
        .join("\n")
}

fn _build_stats(
    x: TargetStat<Vec<Stat>>,
    _stats_map: &mut BTreeMap<&'static str, PrometheusMetric<'static>>,
    _time: Duration,
) {
    let TargetStat {
        kind: _,
        target: _,
        value,
        ..
    } = x;

    for _x in value {}
}

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
            "disk_iosize" => stats_map.get_mut_metric(TargetStatsMetric::DISK_IO_TOTAL),
            "rpc_hist" => stats_map.get_mut_metric(TargetStatsMetric::DISK_IO),
            "pages" => stats_map.get_mut_metric(TargetStatsMetric::PAGES_PER_BULK_RW_TOTAL),
            "discont_pages" => {
                stats_map.get_mut_metric(TargetStatsMetric::DISCONTIGUOUS_PAGES_TOTAL)
            }
            "dio_frags" => stats_map.get_mut_metric(TargetStatsMetric::DISK_IO_FRAGS),
            "discont_blocks" => {
                stats_map.get_mut_metric(TargetStatsMetric::DISCONTIGUOUS_BLOCKS_TOTAL)
            }
            "io_time" => stats_map.get_mut_metric(TargetStatsMetric::IO_TIME_MILLISECONDS_TOTAL),
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

fn build_ost_job_stats(
    x: TargetStat<Option<Vec<JobStatOst>>>,
    stats_map: &mut BTreeMap<&'static str, PrometheusMetric<'static>>,
    time: Duration,
) {
    let TargetStat {
        kind,
        target,
        value,
        ..
    } = x;

    let xs = match value {
        Some(xs) => xs,
        None => return,
    };

    for x in xs {
        let (rs, rmin, rmax, rb, ws, wmin, wmax, wb) =
            jobstatost_inst(&x, kind.deref(), target.deref(), time);

        stats_map
            .get_mut_metric(JobStatOstMetric::READ_SAMPLES)
            .render_and_append_instance(&rs);
        stats_map
            .get_mut_metric(JobStatOstMetric::READ_MIN_SIZE_BYTES)
            .render_and_append_instance(&rmin);
        stats_map
            .get_mut_metric(JobStatOstMetric::READ_MAX_SIZE_BYTES)
            .render_and_append_instance(&rmax);
        stats_map
            .get_mut_metric(JobStatOstMetric::READ_BYTES)
            .render_and_append_instance(&rb);
        stats_map
            .get_mut_metric(JobStatOstMetric::WRITE_SAMPLES)
            .render_and_append_instance(&ws);
        stats_map
            .get_mut_metric(JobStatOstMetric::WRITE_MIN_SIZE_BYTES)
            .render_and_append_instance(&wmin);
        stats_map
            .get_mut_metric(JobStatOstMetric::WRITE_MAX_SIZE_BYTES)
            .render_and_append_instance(&wmax);
        stats_map
            .get_mut_metric(JobStatOstMetric::WRITE_BYTES)
            .render_and_append_instance(&wb);
    }
}

fn build_target_stats(
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
                .get_mut_metric(TargetStatsMetric::INODES_FREE)
                .render_and_append_instance(&x.to_metric_inst(time));
        }
        TargetStats::FilesTotal(x) => {
            stats_map
                .get_mut_metric(TargetStatsMetric::INODES_MAXIMUM)
                .render_and_append_instance(&x.to_metric_inst(time));
        }
        TargetStats::FsType(_) => {}
        TargetStats::BytesAvail(x) => {
            stats_map
                .get_mut_metric(TargetStatsMetric::AVAILABLE_BYTES)
                .render_and_append_instance(&x.to_metric_inst(time));
        }
        TargetStats::BytesFree(x) => {
            stats_map
                .get_mut_metric(TargetStatsMetric::FREE_BYTES)
                .render_and_append_instance(&x.to_metric_inst(time));
        }
        TargetStats::BytesTotal(x) => {
            stats_map
                .get_mut_metric(TargetStatsMetric::CAPACITY_BYTES)
                .render_and_append_instance(&x.to_metric_inst(time));
        }
        TargetStats::NumExports(x) => {
            stats_map
                .get_mut_metric(TargetStatsMetric::EXPORTS_TOTAL)
                .render_and_append_instance(&x.to_metric_inst(time));
        }
        TargetStats::TotDirty(x) => {
            stats_map
                .get_mut_metric(TargetStatsMetric::EXPORTS_DIRTY_TOTAL)
                .render_and_append_instance(&x.to_metric_inst(time));
        }
        TargetStats::TotGranted(x) => {
            stats_map
                .get_mut_metric(TargetStatsMetric::EXPORTS_GRANTED_TOTAL)
                .render_and_append_instance(&x.to_metric_inst(time));
        }
        TargetStats::TotPending(x) => {
            stats_map
                .get_mut_metric(TargetStatsMetric::EXPORTS_PENDING_TOTAL)
                .render_and_append_instance(&x.to_metric_inst(time));
        }
        TargetStats::ContendedLocks(x) => {
            stats_map
                .get_mut_metric(TargetStatsMetric::LOCK_CONTENDED_TOTAL)
                .render_and_append_instance(&x.to_metric_inst(time));
        }
        TargetStats::ContentionSeconds(x) => {
            stats_map
                .get_mut_metric(TargetStatsMetric::LOCK_CONTENTION_SECONDS_TOTAL)
                .render_and_append_instance(&x.to_metric_inst(time));
        }
        TargetStats::ConnectedClients(x) => {
            stats_map
                .get_mut_metric(TargetStatsMetric::CONNECTED_CLIENTS)
                .render_and_append_instance(&x.to_metric_inst(time));
        }

        TargetStats::CtimeAgeLimit(_x) => {}
        TargetStats::EarlyLockCancel(_x) => {}
        TargetStats::FsNames(_x) => {}
        TargetStats::LockCount(x) => {
            stats_map
                .get_mut_metric(TargetStatsMetric::LOCK_COUNT_TOTAL)
                .render_and_append_instance(&x.to_metric_inst(time));
        }
        TargetStats::LockTimeouts(x) => {
            stats_map
                .get_mut_metric(TargetStatsMetric::LOCK_TIMEOUT_TOTAL)
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

fn build_lnet_stats(
    x: LNetStats,
    stats_map: &mut BTreeMap<&'static str, PrometheusMetric<'static>>,
    time: Duration,
) {
    match x {
        LNetStats::SendCount(x) => {
            stats_map
                .get_mut_metric(LNetStatsMetric::SEND_COUNT)
                .render_and_append_instance(&x.to_metric_inst(time));
        }
        LNetStats::RecvCount(x) => {
            stats_map
                .get_mut_metric(LNetStatsMetric::RECEIVE_COUNT)
                .render_and_append_instance(&x.to_metric_inst(time));
        }
        LNetStats::DropCount(x) => {
            stats_map
                .get_mut_metric(LNetStatsMetric::DROP_COUNT)
                .render_and_append_instance(&x.to_metric_inst(time));
        }
    };
}

trait ToMetricInst<T>
where
    T: Num + fmt::Display + fmt::Debug + Copy,
{
    fn to_metric_inst(&self, time: Duration) -> PrometheusInstance<'_, T, Yes>;
}

impl<T> ToMetricInst<T> for TargetStat<T>
where
    T: Num + fmt::Display + fmt::Debug + Copy,
{
    fn to_metric_inst(&self, time: Duration) -> PrometheusInstance<'_, T, Yes> {
        PrometheusInstance::new()
            .with_label("component", self.kind.deref())
            .with_label("target", self.target.deref())
            .with_value(self.value)
            .with_timestamp(time.as_millis())
    }
}

impl<T> ToMetricInst<T> for LNetStat<T>
where
    T: Num + fmt::Display + fmt::Debug + Copy,
{
    fn to_metric_inst(&self, time: Duration) -> PrometheusInstance<'_, T, Yes> {
        PrometheusInstance::new()
            .with_label("nid", self.nid.deref())
            .with_value(self.value)
            .with_timestamp(time.as_millis())
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

type JobStatOstPromInst<'a> = (
    PrometheusInstance<'a, i64, Yes>,
    PrometheusInstance<'a, i64, Yes>,
    PrometheusInstance<'a, i64, Yes>,
    PrometheusInstance<'a, i64, Yes>,
    PrometheusInstance<'a, i64, Yes>,
    PrometheusInstance<'a, i64, Yes>,
    PrometheusInstance<'a, i64, Yes>,
    PrometheusInstance<'a, i64, Yes>,
);

fn jobstatost_inst<'a>(
    x: &'a JobStatOst,
    kind: &'a str,
    target: &'a str,
    time: Duration,
) -> JobStatOstPromInst<'a> {
    let rs = PrometheusInstance::new()
        .with_label("component", kind)
        .with_label("target", target)
        .with_label("jobid", x.job_id.deref())
        .with_value(x.read_bytes.samples)
        .with_timestamp(time.as_millis());
    let rmin = PrometheusInstance::new()
        .with_label("component", kind)
        .with_label("target", target)
        .with_label("jobid", x.job_id.deref())
        .with_value(x.read_bytes.min)
        .with_timestamp(time.as_millis());
    let rmax = PrometheusInstance::new()
        .with_label("component", kind)
        .with_label("target", target)
        .with_label("jobid", x.job_id.deref())
        .with_value(x.read_bytes.max)
        .with_timestamp(time.as_millis());
    let rb = PrometheusInstance::new()
        .with_label("component", kind)
        .with_label("target", target)
        .with_label("jobid", x.job_id.deref())
        .with_value(x.read_bytes.sum)
        .with_timestamp(time.as_millis());
    let ws = PrometheusInstance::new()
        .with_label("component", kind)
        .with_label("target", target)
        .with_label("jobid", x.job_id.deref())
        .with_value(x.write_bytes.samples)
        .with_timestamp(time.as_millis());
    let wmin = PrometheusInstance::new()
        .with_label("component", kind)
        .with_label("target", target)
        .with_label("jobid", x.job_id.deref())
        .with_value(x.write_bytes.min)
        .with_timestamp(time.as_millis());
    let wmax = PrometheusInstance::new()
        .with_label("component", kind)
        .with_label("target", target)
        .with_label("jobid", x.job_id.deref())
        .with_value(x.write_bytes.max)
        .with_timestamp(time.as_millis());
    let wb = PrometheusInstance::new()
        .with_label("component", kind)
        .with_label("target", target)
        .with_label("jobid", x.job_id.deref())
        .with_value(x.write_bytes.sum)
        .with_timestamp(time.as_millis());

    (rs, rmin, rmax, rb, ws, wmin, wmax, wb)
}

#[derive(Debug)]
struct Options;

#[tokio::main]
async fn main() {
    let addr = ([0, 0, 0, 0], 32221).into();

    println!("starting exporter on {addr}");

    render_prometheus(addr, Options, |request, options| async move {
        println!("in our render_prometheus(request == {request:?}, options == {options:?})");

        let output = Command::new("lctl")
            .arg("get_param")
            .args(parser::params())
            .kill_on_drop(true)
            .output()
            .await?;

        let time = SystemTime::now().duration_since(UNIX_EPOCH)?;

        let lctl_output = parse_lctl_output(&output.stdout)?;

        Ok(build_lustre_stats(lctl_output, time))
    })
    .await;
}

#[cfg(test)]
mod tests {
    use crate::build_lustre_stats;
    use std::time::UNIX_EPOCH;

    #[test]
    fn test_stats() {
        let output = include_str!("../fixtures/stats.json");

        let x = serde_json::from_str(output).unwrap();

        let x = build_lustre_stats(x, UNIX_EPOCH.duration_since(UNIX_EPOCH).unwrap());

        insta::assert_display_snapshot!(x);
    }
    #[test]
    fn test_jobstats() {
        let output = include_str!("../fixtures/jobstats.json");

        let x = serde_json::from_str(output).unwrap();

        let x = build_lustre_stats(x, UNIX_EPOCH.duration_since(UNIX_EPOCH).unwrap());

        insta::assert_display_snapshot!(x);
    }
}

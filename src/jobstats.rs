use std::{collections::BTreeMap, ops::Deref, time::Duration};

use crate::{Metric, StatsMapExt};
use casey::{lower, upper};
use lustre_collector::{JobStatMdt, JobStatOst, TargetStat};
use prometheus_exporter_base::{prelude::*, Yes};

static READ_SAMPLES: Metric = Metric {
    name: "job_read_samples_total",
    help: "Total number of reads that have been recorded.",
    r#type: MetricType::Counter,
};
static READ_MIN_SIZE_BYTES: Metric = Metric {
    name: "job_read_minimum_size_bytes",
    help: "The minimum read size in bytes.",
    r#type: MetricType::Gauge,
};
static READ_MAX_SIZE_BYTES: Metric = Metric {
    name: "job_read_maximum_size_bytes",
    help: "The maximum read size in bytes.",
    r#type: MetricType::Gauge,
};
static READ_BYTES: Metric = Metric {
    name: "job_read_bytes_total",
    help: "The total number of bytes that have been read.",
    r#type: MetricType::Counter,
};

static WRITE_SAMPLES: Metric = Metric {
    name: "job_write_samples_total",
    help: "Total number of writes that have been recorded.",
    r#type: MetricType::Counter,
};
static WRITE_MIN_SIZE_BYTES: Metric = Metric {
    name: "job_write_minimum_size_bytes",
    help: "The minimum write size in bytes.",
    r#type: MetricType::Gauge,
};
static WRITE_MAX_SIZE_BYTES: Metric = Metric {
    name: "job_write_maximum_size_bytes",
    help: "The maximum write size in bytes.",
    r#type: MetricType::Gauge,
};
static WRITE_BYTES: Metric = Metric {
    name: "job_write_bytes_total",
    help: "The total number of bytes that have been written.",
    r#type: MetricType::Counter,
};

pub fn build_ost_job_stats(
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
            .get_mut_metric(READ_SAMPLES)
            .render_and_append_instance(&rs);
        stats_map
            .get_mut_metric(READ_MIN_SIZE_BYTES)
            .render_and_append_instance(&rmin);
        stats_map
            .get_mut_metric(READ_MAX_SIZE_BYTES)
            .render_and_append_instance(&rmax);
        stats_map
            .get_mut_metric(READ_BYTES)
            .render_and_append_instance(&rb);
        stats_map
            .get_mut_metric(WRITE_SAMPLES)
            .render_and_append_instance(&ws);
        stats_map
            .get_mut_metric(WRITE_MIN_SIZE_BYTES)
            .render_and_append_instance(&wmin);
        stats_map
            .get_mut_metric(WRITE_MAX_SIZE_BYTES)
            .render_and_append_instance(&wmax);
        stats_map
            .get_mut_metric(WRITE_BYTES)
            .render_and_append_instance(&wb);
    }
}

macro_rules! create_mdt_metrics {
    ($( $name:ident ),* ) =>
        {
            $(
                static $name: Metric = Metric {
                    name: concat!("lustre_", lower!(stringify!($name)), "_total"),
                    help: concat!("Total number of ", stringify!($name) ," operations that have been recorded."),
                    r#type: MetricType::Counter,
                };
            )*
        }
}

macro_rules! build_mdt_metrics {
    ($stats_map:ident, $( $name:ident ),*  ) => {
        $(
        $stats_map
            .get_mut_metric(upper!($name))
            .render_and_append_instance(&$name);
        )*
    };
}

create_mdt_metrics!(
    OPEN,
    CLOSE,
    MKNOD,
    LINK,
    UNLINK,
    MKDIR,
    RMDIR,
    RENAME,
    GETATTR,
    SETATTR,
    GETXATTR,
    SETXATTR,
    STATFS,
    SYNC,
    SAMEDIR_RENAME,
    CROSSDIR_RENAME
);

pub fn build_mdt_job_stats(
    x: TargetStat<Option<Vec<JobStatMdt>>>,
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
        let (
            open,
            close,
            mknod,
            link,
            unlink,
            mkdir,
            rmdir,
            rename,
            getattr,
            setattr,
            getxattr,
            setxattr,
            statfs,
            sync,
            samedir_rename,
            crossdir_rename,
        ) = jobstatmdt_inst(&x, kind.deref(), target.deref(), time);
        build_mdt_metrics!(
            stats_map,
            open,
            close,
            mknod,
            link,
            unlink,
            mkdir,
            rmdir,
            rename,
            getattr,
            setattr,
            getxattr,
            setxattr,
            statfs,
            sync,
            samedir_rename,
            crossdir_rename
        );
    }
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

type JobStatMdtPromInst<'a> = (
    PrometheusInstance<'a, i64, Yes>,
    PrometheusInstance<'a, i64, Yes>,
    PrometheusInstance<'a, i64, Yes>,
    PrometheusInstance<'a, i64, Yes>,
    PrometheusInstance<'a, i64, Yes>,
    PrometheusInstance<'a, i64, Yes>,
    PrometheusInstance<'a, i64, Yes>,
    PrometheusInstance<'a, i64, Yes>,
    PrometheusInstance<'a, i64, Yes>,
    PrometheusInstance<'a, i64, Yes>,
    PrometheusInstance<'a, i64, Yes>,
    PrometheusInstance<'a, i64, Yes>,
    PrometheusInstance<'a, i64, Yes>,
    PrometheusInstance<'a, i64, Yes>,
    PrometheusInstance<'a, i64, Yes>,
    PrometheusInstance<'a, i64, Yes>,
);

macro_rules! mdt_inst {
    ($pm:ident, $kind:ident, $target:ident,  $time:ident, $obj:ident, $( $field:ident ),* ) =>
        {
            (
            $(
                $pm::new()
        .with_label("component", $kind)
        .with_label("target", $target)
        .with_label("jobid", $obj.job_id.deref())
        .with_value($obj.$field.samples)
        .with_timestamp($time.as_millis()),
            )*
            )
        }
}

fn jobstatmdt_inst<'a>(
    x: &'a JobStatMdt,
    kind: &'a str,
    target: &'a str,
    time: Duration,
) -> JobStatMdtPromInst<'a> {
    let x = mdt_inst!(
        PrometheusInstance,
        kind,
        target,
        time,
        x,
        open,
        close,
        mknod,
        link,
        unlink,
        mkdir,
        rmdir,
        rename,
        getattr,
        setattr,
        getxattr,
        setxattr,
        statfs,
        sync,
        samedir_rename,
        crossdir_rename
    );
    x
}

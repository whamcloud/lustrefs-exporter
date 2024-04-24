use std::{collections::BTreeMap, ops::Deref};

use crate::{LabelProm, Metric, StatsMapExt};
use lustre_collector::{JobStatMdt, JobStatOst, TargetStat};
use prometheus_exporter_base::{prelude::*, Yes};

static READ_SAMPLES: Metric = Metric {
    name: "lustre_job_read_samples_total",
    help: "Total number of reads that have been recorded.",
    r#type: MetricType::Counter,
};
static READ_MIN_SIZE_BYTES: Metric = Metric {
    name: "lustre_job_read_minimum_size_bytes",
    help: "The minimum read size in bytes.",
    r#type: MetricType::Gauge,
};
static READ_MAX_SIZE_BYTES: Metric = Metric {
    name: "lustre_job_read_maximum_size_bytes",
    help: "The maximum read size in bytes.",
    r#type: MetricType::Gauge,
};
static READ_BYTES: Metric = Metric {
    name: "lustre_job_read_bytes_total",
    help: "The total number of bytes that have been read.",
    r#type: MetricType::Counter,
};

static WRITE_SAMPLES: Metric = Metric {
    name: "lustre_job_write_samples_total",
    help: "Total number of writes that have been recorded.",
    r#type: MetricType::Counter,
};
static WRITE_MIN_SIZE_BYTES: Metric = Metric {
    name: "lustre_job_write_minimum_size_bytes",
    help: "The minimum write size in bytes.",
    r#type: MetricType::Gauge,
};
static WRITE_MAX_SIZE_BYTES: Metric = Metric {
    name: "lustre_job_write_maximum_size_bytes",
    help: "The maximum write size in bytes.",
    r#type: MetricType::Gauge,
};
static WRITE_BYTES: Metric = Metric {
    name: "lustre_job_write_bytes_total",
    help: "The total number of bytes that have been written.",
    r#type: MetricType::Counter,
};

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
) -> JobStatOstPromInst<'a> {
    let rs = PrometheusInstance::new()
        .with_label("component", kind)
        .with_label("target", target)
        .with_label("jobid", x.job_id.deref())
        .with_value(x.read_bytes.samples);
    let rmin = PrometheusInstance::new()
        .with_label("component", kind)
        .with_label("target", target)
        .with_label("jobid", x.job_id.deref())
        .with_value(x.read_bytes.min);
    let rmax = PrometheusInstance::new()
        .with_label("component", kind)
        .with_label("target", target)
        .with_label("jobid", x.job_id.deref())
        .with_value(x.read_bytes.max);
    let rb = PrometheusInstance::new()
        .with_label("component", kind)
        .with_label("target", target)
        .with_label("jobid", x.job_id.deref())
        .with_value(x.read_bytes.sum);
    let ws = PrometheusInstance::new()
        .with_label("component", kind)
        .with_label("target", target)
        .with_label("jobid", x.job_id.deref())
        .with_value(x.write_bytes.samples);
    let wmin = PrometheusInstance::new()
        .with_label("component", kind)
        .with_label("target", target)
        .with_label("jobid", x.job_id.deref())
        .with_value(x.write_bytes.min);
    let wmax = PrometheusInstance::new()
        .with_label("component", kind)
        .with_label("target", target)
        .with_label("jobid", x.job_id.deref())
        .with_value(x.write_bytes.max);
    let wb = PrometheusInstance::new()
        .with_label("component", kind)
        .with_label("target", target)
        .with_label("jobid", x.job_id.deref())
        .with_value(x.write_bytes.sum);

    (rs, rmin, rmax, rb, ws, wmin, wmax, wb)
}

pub fn build_ost_job_stats(
    x: TargetStat<Option<Vec<JobStatOst>>>,
    stats_map: &mut BTreeMap<&'static str, PrometheusMetric<'static>>,
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
            jobstatost_inst(&x, kind.to_prom_label(), target.deref());

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
    PrometheusInstance<'a, i64, Yes>,
    PrometheusInstance<'a, i64, Yes>,
    PrometheusInstance<'a, i64, Yes>,
    Option<PrometheusInstance<'a, i64, Yes>>,
    Option<PrometheusInstance<'a, i64, Yes>>,
);

fn jobstatmdt_inst<'a>(
    x: &'a JobStatMdt,
    kind: &'a str,
    target: &'a str,
) -> JobStatMdtPromInst<'a> {
    let JobStatMdt {
        job_id,
        snapshot_time: _,
        start_time: _,
        elapsed_time: _,
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
        read_bytes,
        write_bytes,
        punch,
        parallel_rename_dir,
        parallel_rename_file,
    } = x;

    (
        PrometheusInstance::new()
            .with_label("component", kind)
            .with_label("target", target)
            .with_label("jobid", job_id.deref())
            .with_label("operation", "open")
            .with_value(open.samples),
        PrometheusInstance::new()
            .with_label("component", kind)
            .with_label("target", target)
            .with_label("jobid", job_id.deref())
            .with_label("operation", "close")
            .with_value(close.samples),
        PrometheusInstance::new()
            .with_label("component", kind)
            .with_label("target", target)
            .with_label("jobid", job_id.deref())
            .with_label("operation", "mknod")
            .with_value(mknod.samples),
        PrometheusInstance::new()
            .with_label("component", kind)
            .with_label("target", target)
            .with_label("jobid", job_id.deref())
            .with_label("operation", "link")
            .with_value(link.samples),
        PrometheusInstance::new()
            .with_label("component", kind)
            .with_label("target", target)
            .with_label("jobid", job_id.deref())
            .with_label("operation", "unlink")
            .with_value(unlink.samples),
        PrometheusInstance::new()
            .with_label("component", kind)
            .with_label("target", target)
            .with_label("jobid", job_id.deref())
            .with_label("operation", "mkdir")
            .with_value(mkdir.samples),
        PrometheusInstance::new()
            .with_label("component", kind)
            .with_label("target", target)
            .with_label("jobid", job_id.deref())
            .with_label("operation", "rmdir")
            .with_value(rmdir.samples),
        PrometheusInstance::new()
            .with_label("component", kind)
            .with_label("target", target)
            .with_label("jobid", job_id.deref())
            .with_label("operation", "rename")
            .with_value(rename.samples),
        PrometheusInstance::new()
            .with_label("component", kind)
            .with_label("target", target)
            .with_label("jobid", job_id.deref())
            .with_label("operation", "getattr")
            .with_value(getattr.samples),
        PrometheusInstance::new()
            .with_label("component", kind)
            .with_label("target", target)
            .with_label("jobid", job_id.deref())
            .with_label("operation", "setattr")
            .with_value(setattr.samples),
        PrometheusInstance::new()
            .with_label("component", kind)
            .with_label("target", target)
            .with_label("jobid", job_id.deref())
            .with_label("operation", "getxattr")
            .with_value(getxattr.samples),
        PrometheusInstance::new()
            .with_label("component", kind)
            .with_label("target", target)
            .with_label("jobid", job_id.deref())
            .with_label("operation", "setxattr")
            .with_value(setxattr.samples),
        PrometheusInstance::new()
            .with_label("component", kind)
            .with_label("target", target)
            .with_label("jobid", job_id.deref())
            .with_label("operation", "statfs")
            .with_value(statfs.samples),
        PrometheusInstance::new()
            .with_label("component", kind)
            .with_label("target", target)
            .with_label("jobid", job_id.deref())
            .with_label("operation", "sync")
            .with_value(sync.samples),
        PrometheusInstance::new()
            .with_label("component", kind)
            .with_label("target", target)
            .with_label("jobid", job_id.deref())
            .with_label("operation", "samedir_rename")
            .with_value(samedir_rename.samples),
        PrometheusInstance::new()
            .with_label("component", kind)
            .with_label("target", target)
            .with_label("jobid", job_id.deref())
            .with_label("operation", "crossdir_rename")
            .with_value(crossdir_rename.samples),
        PrometheusInstance::new()
            .with_label("component", kind)
            .with_label("target", target)
            .with_label("jobid", job_id.deref())
            .with_label("operation", "punch")
            .with_value(punch.samples),
        PrometheusInstance::new()
            .with_label("component", kind)
            .with_label("target", target)
            .with_label("jobid", job_id.deref())
            .with_label("operation", "read_bytes")
            .with_value(read_bytes.samples),
        PrometheusInstance::new()
            .with_label("component", kind)
            .with_label("target", target)
            .with_label("jobid", job_id.deref())
            .with_label("operation", "write_bytes")
            .with_value(write_bytes.samples),
        parallel_rename_dir.as_ref().map(|parallel_rename_dir| {
            PrometheusInstance::new()
                .with_label("component", kind)
                .with_label("target", target)
                .with_label("jobid", job_id.deref())
                .with_label("operation", "parallel_rename_dir")
                .with_value(parallel_rename_dir.samples)
        }),
        parallel_rename_file.as_ref().map(|parallel_rename_file| {
            PrometheusInstance::new()
                .with_label("component", kind)
                .with_label("target", target)
                .with_label("jobid", job_id.deref())
                .with_label("operation", "parallel_rename_file")
                .with_value(parallel_rename_file.samples)
        }),
    )
}

static MDT_JOBSTATS_SAMPLES: Metric = Metric {
    name: "lustre_job_stats_total",
    help: "Number of operations the filesystem has performed, recorded by jobstats.",
    r#type: MetricType::Counter,
};

pub fn build_mdt_job_stats(
    x: TargetStat<Option<Vec<JobStatMdt>>>,
    stats_map: &mut BTreeMap<&'static str, PrometheusMetric<'static>>,
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
            punch,
            read_bytes,
            write_bytes,
            parallel_rename_dir,
            parallel_rename_file,
        ) = jobstatmdt_inst(&x, kind.to_prom_label(), target.deref());

        stats_map
            .get_mut_metric(MDT_JOBSTATS_SAMPLES)
            .render_and_append_instance(&open)
            .render_and_append_instance(&close)
            .render_and_append_instance(&mknod)
            .render_and_append_instance(&link)
            .render_and_append_instance(&unlink)
            .render_and_append_instance(&mkdir)
            .render_and_append_instance(&rmdir)
            .render_and_append_instance(&rename)
            .render_and_append_instance(&getattr)
            .render_and_append_instance(&setattr)
            .render_and_append_instance(&getxattr)
            .render_and_append_instance(&setxattr)
            .render_and_append_instance(&statfs)
            .render_and_append_instance(&sync)
            .render_and_append_instance(&samedir_rename)
            .render_and_append_instance(&crossdir_rename)
            .render_and_append_instance(&punch)
            .render_and_append_instance(&read_bytes)
            .render_and_append_instance(&write_bytes);
        if let Some(parallel_rename_dir) = parallel_rename_dir {
            stats_map
                .get_mut_metric(MDT_JOBSTATS_SAMPLES)
                .render_and_append_instance(&parallel_rename_dir);
        }
        if let Some(parallel_rename_file) = parallel_rename_file {
            stats_map
                .get_mut_metric(MDT_JOBSTATS_SAMPLES)
                .render_and_append_instance(&parallel_rename_file);
        }
    }
}

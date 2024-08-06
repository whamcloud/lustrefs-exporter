use crate::{Error, LabelProm, Metric, StatsMapExt};
use compact_str::CompactString;
use lustre_collector::{JobStatMdt, JobStatOst, TargetStat, TargetVariant};
use prometheus_exporter_base::{prelude::*, Yes};
use regex::Regex;
use std::{collections::BTreeMap, io::BufRead, ops::Deref, sync::LazyLock};
use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    task::JoinHandle,
};

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

    let create = PrometheusInstance::new()
        .with_label("component", kind)
        .with_label("target", target)
        .with_label("jobid", x.job_id.deref())
        .with_label("operation", "create")
        .with_value(x.create.samples);
    let destroy = PrometheusInstance::new()
        .with_label("component", kind)
        .with_label("target", target)
        .with_label("jobid", x.job_id.deref())
        .with_label("operation", "destroy")
        .with_value(x.destroy.samples);
    let get_info = PrometheusInstance::new()
        .with_label("component", kind)
        .with_label("target", target)
        .with_label("jobid", x.job_id.deref())
        .with_label("operation", "get_info")
        .with_value(x.get_info.samples);
    let getattr = PrometheusInstance::new()
        .with_label("component", kind)
        .with_label("target", target)
        .with_label("jobid", x.job_id.deref())
        .with_label("operation", "getattr")
        .with_value(x.getattr.samples);
    let punch = PrometheusInstance::new()
        .with_label("component", kind)
        .with_label("target", target)
        .with_label("jobid", x.job_id.deref())
        .with_label("operation", "punch")
        .with_value(x.punch.samples);
    let quotactl = PrometheusInstance::new()
        .with_label("component", kind)
        .with_label("target", target)
        .with_label("jobid", x.job_id.deref())
        .with_label("operation", "quotactl")
        .with_value(x.quotactl.samples);
    let set_info = PrometheusInstance::new()
        .with_label("component", kind)
        .with_label("target", target)
        .with_label("jobid", x.job_id.deref())
        .with_label("operation", "set_info")
        .with_value(x.set_info.samples);
    let setattr = PrometheusInstance::new()
        .with_label("component", kind)
        .with_label("target", target)
        .with_label("jobid", x.job_id.deref())
        .with_label("operation", "setattr")
        .with_value(x.setattr.samples);
    let statfs = PrometheusInstance::new()
        .with_label("component", kind)
        .with_label("target", target)
        .with_label("jobid", x.job_id.deref())
        .with_label("operation", "statfs")
        .with_value(x.statfs.samples);
    let sync = PrometheusInstance::new()
        .with_label("component", kind)
        .with_label("target", target)
        .with_label("jobid", x.job_id.deref())
        .with_label("operation", "sync")
        .with_value(x.sync.samples);

    (
        rs, rmin, rmax, rb, ws, wmin, wmax, wb, create, destroy, get_info, getattr, punch,
        quotactl, set_info, setattr, statfs, sync,
    )
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
        let (
            rs,
            rmin,
            rmax,
            rb,
            ws,
            wmin,
            wmax,
            wb,
            create,
            destroy,
            get_info,
            getattr,
            punch,
            quotactl,
            set_info,
            setattr,
            statfs,
            sync,
        ) = jobstatost_inst(&x, kind.to_prom_label(), target.deref());

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

        stats_map
            .get_mut_metric(MDT_JOBSTATS_SAMPLES)
            .render_and_append_instance(&create)
            .render_and_append_instance(&destroy)
            .render_and_append_instance(&get_info)
            .render_and_append_instance(&getattr)
            .render_and_append_instance(&punch)
            .render_and_append_instance(&quotactl)
            .render_and_append_instance(&set_info)
            .render_and_append_instance(&setattr)
            .render_and_append_instance(&statfs)
            .render_and_append_instance(&sync);
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

#[derive(Debug)]
enum State {
    Empty,
    Target(String),
    TargetJob(String, String),
    TargetJobStats(String, String, Vec<String>),
}

pub fn jobstats_stream<R: BufRead + std::marker::Send + 'static>(
    f: R,
) -> (JoinHandle<Result<(), Error>>, Receiver<CompactString>) {
    let (tx, rx) = mpsc::channel(200);

    let x = tokio::task::spawn_blocking(move || {
        let mut state = State::Empty;

        for line in f.lines() {
            let line = line?;

            match state {
                _ if line == "job_stats:" || line.starts_with("  snapshot_time:") => continue,
                State::Empty if line.starts_with("obdfilter") || line.starts_with("mdt.") => {
                    state = State::Target(line);
                }
                State::Target(x) if line.starts_with("- job_id:") => {
                    state = State::TargetJob(x, line);
                }
                State::TargetJob(target, job) if line.starts_with("  ") => {
                    let mut xs = Vec::with_capacity(10);

                    xs.push(line);

                    state = State::TargetJobStats(target, job, xs);
                }
                State::TargetJobStats(target, job, mut stats) if line.starts_with("  ") => {
                    stats.push(line);

                    state = State::TargetJobStats(target, job, stats);
                }
                State::TargetJobStats(target, job, stats) if line.starts_with("- job_id:") => {
                    render_stat(tx.clone(), &target, job, stats)?;

                    state = State::TargetJob(target, line);
                }
                State::TargetJobStats(target, job, stats)
                    if line.starts_with("obdfilter") || line.starts_with("mdt.") =>
                {
                    render_stat(tx.clone(), &target, job, stats)?;

                    state = State::Target(line);
                }
                x => {
                    tracing::debug!("Unexpected line: {line}, state: {x:?}");

                    break;
                }
            }
        }

        Ok(())
    });

    (x, rx)
}

static TARGET: LazyLock<regex::Regex> = LazyLock::new(|| {
    Regex::new(r#"^(obdfilter|mdt)\.([a-zA-Z0-9_-]+)\.job_stats=$"#).expect("A Well-formed regex")
});

static JOB_STAT: LazyLock<regex::Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
        ^\ \ (?<stat>[a-z_]+):\ +\{         # 1. stat name
        \ samples:\ +(?<sample>[0-9]+),     # 2. sample value
        \ unit:\ +([a-z]+),                 # 3. unit value
        \ min:\ +(?<min>[0-9]+),            # 4. min value
        \ max:\ +(?<max>[0-9]+),            # 5. max value
        \ sum:\ +(?<sum>[0-9]+),            # 6. sum value
        \ sumsq:\ +(?<sumsq>[0-9]+)         # 7. sumsq value
",
    )
    .expect("A Well-formed regex")
});

fn handle_sample(
    tx: &Sender<CompactString>,
    stat_name: &str,
    target: &str,
    job: &str,
    kind: &TargetVariant,
    value: &str,
) {
    let s = compact_str::format_compact!(
        "{}{{operation=\"{}\",component=\"{}\",target=\"{target}\",jobid=\"{job}\"}} {value}",
        MDT_JOBSTATS_SAMPLES.name,
        stat_name,
        kind.to_prom_label()
    );

    _ = tx.blocking_send(s);
}

fn render_stat(
    tx: Sender<CompactString>,
    target: &str,
    job: String,
    stats: Vec<String>,
) -> Result<(), Error> {
    let (_, [kind, target]) = TARGET
        .captures(target)
        .ok_or_else(|| Error::NoCap("target", target.to_owned()))?
        .extract();

    let kind = if kind == "obdfilter" {
        TargetVariant::Ost
    } else {
        TargetVariant::Mdt
    };

    let job = job.replace("- job_id:", "").replace('"', "");
    let jobid = job.trim();

    for stat in stats {
        let cap = JOB_STAT
            .captures(&stat)
            .ok_or_else(|| Error::NoCap("job_stat", stat.to_owned()))?;

        let (_, [stat_name, samples, _unit, min, max, sum, _sumsq]) = cap.extract();

        if kind == TargetVariant::Ost {
            match stat_name {
                "read_bytes" => {
                    for (value, metric) in [
                        (samples, READ_SAMPLES),
                        (min, READ_MIN_SIZE_BYTES),
                        (max, READ_MAX_SIZE_BYTES),
                        (sum, READ_BYTES),
                    ] {
                        let s = compact_str::format_compact!(
                                "{}{{operation=\"{}\",component=\"{}\",target=\"{target}\",jobid=\"{jobid}\"}} {value}",
                                metric.name,
                                stat_name,
                                kind.to_prom_label()
                            );

                        _ = tx.blocking_send(s);
                    }
                }
                "write_bytes" => {
                    for (value, metric) in [
                        (samples, WRITE_SAMPLES),
                        (min, WRITE_MIN_SIZE_BYTES),
                        (max, WRITE_MAX_SIZE_BYTES),
                        (sum, WRITE_BYTES),
                    ] {
                        let s = compact_str::format_compact!(
                            "{}{{operation=\"{}\",component=\"{}\",target=\"{target}\",jobid=\"{jobid}\"}} {value}",
                            metric.name,
                            stat_name,
                            kind.to_prom_label()
                        );

                        _ = tx.blocking_send(s);
                    }
                }
                "getattr" | "setattr" | "punch" | "sync" | "destroy" | "create" | "statfs"
                | "get_info" | "set_info" | "quotactl" => {
                    handle_sample(&tx, stat_name, target, jobid, &kind, samples);
                }
                _ => continue,
            };
        } else if kind == TargetVariant::Mdt {
            match stat_name {
                "open"
                | "close"
                | "mknod"
                | "link"
                | "unlink"
                | "mkdir"
                | "rmdir"
                | "rename"
                | "getattr"
                | "setattr"
                | "getxattr"
                | "setxattr"
                | "statfs"
                | "sync"
                | "samedir_rename"
                | "parallel_rename_file"
                | "parallel_rename_dir"
                | "crossdir_rename"
                | "read"
                | "write"
                | "read_bytes"
                | "write_bytes"
                | "punch"
                | "migrate" => {
                    handle_sample(&tx, stat_name, target, jobid, &kind, samples);
                }
                _ => continue,
            };
        }
    }

    Ok(())
}

#[cfg(test)]
pub mod tests {
    use crate::jobstats::jobstats_stream;
    use std::{fs::File, io::BufReader};

    #[tokio::test(flavor = "multi_thread")]
    async fn parse_large_yaml() {
        let f = File::open("fixtures/ds86.txt").unwrap();

        let f = BufReader::with_capacity(128 * 1_024, f);

        let (fut, mut rx) = jobstats_stream(f);

        let mut cnt = 0;

        while rx.recv().await.is_some() {
            cnt += 1;
        }

        fut.await.unwrap().unwrap();

        assert_eq!(cnt, 3_524_622);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn parse_large_yaml2() {
        let f = File::open("fixtures/co-vm03.txt").unwrap();

        let f = BufReader::with_capacity(128 * 1_024, f);

        let (fut, mut rx) = jobstats_stream(f);

        let mut cnt = 0;

        while rx.recv().await.is_some() {
            cnt += 1;
        }

        fut.await.unwrap().unwrap();

        assert_eq!(cnt, 884_988);
    }
}

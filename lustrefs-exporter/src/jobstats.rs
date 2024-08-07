use crate::{Error, LabelProm, Metric};
use compact_str::{format_compact, CompactString, ToCompactString};
use lustre_collector::TargetVariant;
use prometheus_exporter_base::MetricType;
use regex::Regex;
use std::{io::BufRead, sync::LazyLock};
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

static MDT_JOBSTATS_SAMPLES: Metric = Metric {
    name: "lustre_job_stats_total",
    help: "Number of operations the filesystem has performed, recorded by jobstats.",
    r#type: MetricType::Counter,
};

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
                ref x => {
                    tracing::debug!("Unexpected line: {line}, state: {x:?}");

                    break;
                }
            }
        }

        if let State::TargetJobStats(target, job, stats) = state {
            render_stat(tx.clone(), &target, job, stats)?;
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

fn send_stat(
    tx: &Sender<CompactString>,
    name: &str,
    stat_name: &str,
    target: &str,
    job: &str,
    kind: &TargetVariant,
    value: &str,
) {
    _ = tx.blocking_send(name.to_compact_string());

    _ = tx.blocking_send("{operation=".to_compact_string());

    _ = tx.blocking_send(format_compact!("\"{stat_name}\","));

    _ = tx.blocking_send(format_compact!("component=\"{}\",", kind.to_prom_label()));

    _ = tx.blocking_send(format_compact!("target=\"{target}\","));

    _ = tx.blocking_send(format_compact!("jobid=\"{job}\"}} {value}\n"));
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
                        send_stat(&tx, metric.name, stat_name, target, jobid, &kind, value);
                    }
                }
                "write_bytes" => {
                    for (value, metric) in [
                        (samples, WRITE_SAMPLES),
                        (min, WRITE_MIN_SIZE_BYTES),
                        (max, WRITE_MAX_SIZE_BYTES),
                        (sum, WRITE_BYTES),
                    ] {
                        send_stat(&tx, metric.name, stat_name, target, jobid, &kind, value);
                    }
                }
                "getattr" | "setattr" | "punch" | "sync" | "destroy" | "create" | "statfs"
                | "get_info" | "set_info" | "quotactl" => {
                    send_stat(
                        &tx,
                        MDT_JOBSTATS_SAMPLES.name,
                        stat_name,
                        target,
                        jobid,
                        &kind,
                        samples,
                    );
                }
                x => {
                    tracing::debug!("Unhandled OST jobstats stats: {x}");
                    continue
                },
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
                    send_stat(
                        &tx,
                        MDT_JOBSTATS_SAMPLES.name,
                        stat_name,
                        target,
                        jobid,
                        &kind,
                        samples,
                    );
                }
                x => {
                    tracing::debug!("Unhandled MDT jobstats stats: {x}");
                    continue
                },
            };
        }
    }

    Ok(())
}

#[cfg(test)]
pub mod tests {
    use const_format::{formatcp, str_repeat};

    use crate::jobstats::jobstats_stream;
    use std::{fs::File, io::BufReader};

    #[tokio::test(flavor = "multi_thread")]
    async fn parse_larger_yaml() {
        let f = File::open("fixtures/ds86.txt").unwrap();

        let f = BufReader::with_capacity(128 * 1_024, f);

        let (fut, mut rx) = jobstats_stream(f);

        let mut cnt = 0;

        while rx.recv().await.is_some() {
            cnt += 1;
        }

        fut.await.unwrap().unwrap();

        assert_eq!(cnt, 21_147_876);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn parse_large_yaml() {
        let f = File::open("fixtures/co-vm03.txt").unwrap();

        let f = BufReader::with_capacity(128 * 1_024, f);

        let (fut, mut rx) = jobstats_stream(f);

        let mut cnt = 0;

        while rx.recv().await.is_some() {
            cnt += 1;
        }

        fut.await.unwrap().unwrap();

        assert_eq!(cnt, 5_310_036);
    }

    const JOBSTAT_JOB: &str = r#"
- job_id:          "FAKE_JOB"
  snapshot_time:   1720516680
  read_bytes:      { samples:           0, unit: bytes, min:        0, max:        0, sum:                0, sumsq:                  0 }
  write_bytes:     { samples:          52, unit: bytes, min:     4096, max:   475136, sum:          5468160, sumsq:      1071040692224 }
  read:            { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  write:           { samples:          52, unit: usecs, min:       12, max:    40081, sum:           692342, sumsq:        17432258604 }
  getattr:         { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  setattr:         { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  punch:           { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  sync:            { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  destroy:         { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  create:          { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  statfs:          { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  get_info:        { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  set_info:        { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  quotactl:        { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  prealloc:        { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }"#;

    const INPUT_10_JOBS: &str = formatcp!(
        r#"obdfilter.ds002-OST0000.job_stats=
job_stats:{}"#,
        str_repeat!(JOBSTAT_JOB, 10)
    );

    #[tokio::test(flavor = "multi_thread")]
    async fn parse_synthetic_yaml() {
        let f = BufReader::with_capacity(128 * 1_024, INPUT_10_JOBS.as_bytes());

        let (fut, mut rx) = jobstats_stream(f);

        let mut output = String::with_capacity(10 * 2 * JOBSTAT_JOB.len());

        while let Some(x) = rx.recv().await {
            output.push_str(x.as_str());
        }

        fut.await.unwrap().unwrap();

        assert_eq!(
            output.lines().count(),
            (4 + // 4 metrics per read_bytes
             4 + // 4 metrics per write_bytes
             10) // 10 metrics for "getattr" | "setattr" | "punch" | "sync" | "destroy" | "create" | "statfs" | "get_info" | "set_info" | "quotactl"
             * 10
        );
    }
}

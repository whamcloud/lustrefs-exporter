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
) -> (JoinHandle<()>, Receiver<CompactString>) {
    let (tx, rx) = mpsc::channel(200);

    enum LoopInstruction {
        Noop,
        Return,
    }

    fn handle_line(
        tx: &Sender<CompactString>,
        maybe_line: Result<String, Error>,
        mut state: State,
    ) -> Result<(State, LoopInstruction), Error> {
        let line = maybe_line?;

        match state {
            _ if line == "job_stats:"
                || line.starts_with("  start_time:")
                || line.starts_with("  elapsed_time:")
                || line.starts_with("  snapshot_time:") =>
            {
                return Ok((state, LoopInstruction::Noop))
            }
            State::Empty | State::Target(_)
                if line.starts_with("obdfilter") || line.starts_with("mdt.") =>
            {
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
                render_stat(tx, &target, job, stats)?;

                state = State::TargetJob(target, line);
            }
            State::TargetJobStats(target, job, stats)
                if line.starts_with("obdfilter") || line.starts_with("mdt.") =>
            {
                render_stat(tx, &target, job, stats)?;

                state = State::Target(line);
            }
            x => {
                tracing::debug!("Unexpected line: {line}, state: {x:?}");
                return Ok((x, LoopInstruction::Return));
            }
        }

        Ok((state, LoopInstruction::Noop))
    }

    let x = tokio::task::spawn_blocking(move || {
        let mut state = State::Empty;

        // Send a new line to make sure we are printing stats with a separating empty line
        _ = tx.blocking_send("\n".to_compact_string());

        for line in f.lines() {
            let r = handle_line(&tx, line.map_err(Error::Io), state);

            match r {
                Ok((new_state, LoopInstruction::Noop)) => state = new_state,
                Ok((_, LoopInstruction::Return)) => return,
                Err(e) => {
                    tracing::debug!("Unexpected error processing jobstats lines: {e}");

                    return;
                }
            }
        }

        if let State::TargetJobStats(target, job, stats) = state {
            if let Err(e) = render_stat(&tx, &target, job, stats) {
                tracing::debug!("Unexpected error processing jobstats lines: {e}");
            };
        }
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
    tx: &Sender<CompactString>,
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
                        send_stat(tx, metric.name, stat_name, target, jobid, &kind, value);
                    }
                }
                "write_bytes" => {
                    for (value, metric) in [
                        (samples, WRITE_SAMPLES),
                        (min, WRITE_MIN_SIZE_BYTES),
                        (max, WRITE_MAX_SIZE_BYTES),
                        (sum, WRITE_BYTES),
                    ] {
                        send_stat(tx, metric.name, stat_name, target, jobid, &kind, value);
                    }
                }
                "getattr" | "setattr" | "punch" | "sync" | "destroy" | "create" | "statfs"
                | "get_info" | "set_info" | "quotactl" => {
                    send_stat(
                        tx,
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
                    continue;
                }
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
                        tx,
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
                    continue;
                }
            };
        }
    }

    Ok(())
}

// pub mod opentelemetry {
//     use compact_str::CompactString;
//     use lustre_collector::TargetVariant;
//     use opentelemetry::{
//         metrics::{Counter, Gauge, Meter},
//         KeyValue,
//     };
//     use std::io::BufRead;

//     #[derive(Debug)]
//     pub struct OpenTelemetryMetricsJobstats {
//         pub read_samples_total: Counter<u64>,
//         pub read_minimum_size_bytes: Gauge<u64>,
//         pub read_maximum_size_bytes: Gauge<u64>,
//         pub read_bytes_total: Counter<u64>,
//         pub write_samples_total: Counter<u64>,
//         pub write_minimum_size_bytes: Gauge<u64>,
//         pub write_maximum_size_bytes: Gauge<u64>,
//         pub write_bytes_total: Counter<u64>,
//         pub stats_total: Counter<u64>,
//     }

//     impl OpenTelemetryMetricsJobstats {
//         pub fn new(meter: &Meter) -> Self {
//             OpenTelemetryMetricsJobstats {
//                 read_samples_total: meter
//                     .u64_counter("lustre_job_read_samples_total")
//                     .with_description("Total number of reads that have been recorded.")
//                     .build(),
//                 read_minimum_size_bytes: meter
//                     .u64_gauge("lustre_job_read_minimum_size_bytes")
//                     .with_description("The minimum read size in bytes.")
//                     .build(),
//                 read_maximum_size_bytes: meter
//                     .u64_gauge("lustre_job_read_maximum_size_bytes")
//                     .with_description("The maximum read size in bytes.")
//                     .build(),
//                 read_bytes_total: meter
//                     .u64_counter("lustre_job_read_bytes_total")
//                     .with_description("The total number of bytes that have been read.")
//                     .build(),
//                 write_samples_total: meter
//                     .u64_counter("lustre_job_write_samples_total")
//                     .with_description("Total number of writes that have been recorded.")
//                     .build(),
//                 write_minimum_size_bytes: meter
//                     .u64_gauge("lustre_job_write_minimum_size_bytes")
//                     .with_description("The minimum write size in bytes.")
//                     .build(),
//                 write_maximum_size_bytes: meter
//                     .u64_gauge("lustre_job_write_maximum_size_bytes")
//                     .with_description("The maximum write size in bytes.")
//                     .build(),
//                 write_bytes_total: meter
//                     .u64_counter("lustre_job_write_bytes_total")
//                     .with_description("The total number of bytes that have been written.")
//                     .build(),
//                 stats_total: meter
//                     .u64_counter("lustre_job_stats_total")
//                     .with_description("Number of operations the filesystem has performed, recorded by jobstats.")
//                     .build(),
//             }
//         }
//     }
// }

//     pub fn record_jobstat(
//         otel_jobstats: &OpenTelemetryMetricsJobstats,
//         stat_name: &str,
//         target: &str,
//         jobid: &str,
//         kind: &TargetVariant,
//         samples: u64,
//         min: u64,
//         max: u64,
//         sum: u64,
//     ) {
//         let base_labels = &[
//             KeyValue::new("operation", stat_name.to_string()),
//             KeyValue::new("component", kind.to_prom_label().to_string()),
//             KeyValue::new("target", target.to_string()),
//             KeyValue::new("jobid", jobid.to_string()),
//         ];

//         if kind == &TargetVariant::Ost {
//             match stat_name {
//                 "read_bytes" => {
//                     otel_jobstats.read_samples_total.add(samples, base_labels);
//                     otel_jobstats.read_minimum_size_bytes.record(min, base_labels);
//                     otel_jobstats.read_maximum_size_bytes.record(max, base_labels);
//                     otel_jobstats.read_bytes_total.add(sum, base_labels);
//                 }
//                 "write_bytes" => {
//                     otel_jobstats.write_samples_total.add(samples, base_labels);
//                     otel_jobstats.write_minimum_size_bytes.record(min, base_labels);
//                     otel_jobstats.write_maximum_size_bytes.record(max, base_labels);
//                     otel_jobstats.write_bytes_total.add(sum, base_labels);
//                 }
//                 "getattr" | "setattr" | "punch" | "sync" | "destroy" | "create" | "statfs"
//                 | "get_info" | "set_info" | "quotactl" => {
//                     otel_jobstats.stats_total.add(samples, base_labels);
//                 }
//                 _ => {
//                     // Unhandled OST jobstats stats
//                 }
//             }
//         } else if kind == &TargetVariant::Mdt {
//             match stat_name {
//                 "open"
//                 | "close"
//                 | "mknod"
//                 | "link"
//                 | "unlink"
//                 | "mkdir"
//                 | "rmdir"
//                 | "rename"
//                 | "getattr"
//                 | "setattr"
//                 | "getxattr"
//                 | "setxattr"
//                 | "statfs"
//                 | "sync"
//                 | "samedir_rename"
//                 | "parallel_rename_file"
//                 | "parallel_rename_dir"
//                 | "crossdir_rename"
//                 | "read"
//                 | "write"
//                 | "read_bytes"
//                 | "write_bytes"
//                 | "punch"
//                 | "migrate" => {
//                     otel_jobstats.stats_total.add(samples, base_labels);
//                 }
//                 _ => {
//                     // Unhandled MDT jobstats stats
//                 }
//             }
//         }
//     }

//     pub fn process_jobstat_stream<R: BufRead + std::marker::Send + 'static>(
//         stream: R,
//         otel_jobstats: &OpenTelemetryMetricsJobstats,
//     ) {
//         use super::{jobstats_stream, JOB_STAT, TARGET};
//         use tokio::sync::mpsc::error::TryRecvError;

//         let (handle, mut rx) = jobstats_stream(stream);

//         // Process stats from the channel
//         tokio::task::spawn_blocking(move || {
//             let mut target = String::new();
//             let mut jobid = String::new();
//             let mut kind = TargetVariant::Ost; // Default value

//             loop {
//                 match rx.try_recv() {
//                     Ok(line) => {
//                         let line_str = line.as_str();

//                         // Process target line
//                         if let Some(captures) = TARGET.captures(line_str) {
//                             let (_, [kind_str, target_str]) = captures.extract();
//                             target = target_str.to_string();
//                             kind = if kind_str == "obdfilter" {
//                                 TargetVariant::Ost
//                             } else {
//                                 TargetVariant::Mdt
//                             };
//                             continue;
//                         }

//                         // Process job id line
//                         if line_str.starts_with("- job_id:") {
//                             jobid = line_str.replace("- job_id:", "").replace('"', "").trim().to_string();
//                             continue;
//                         }

//                         // Process stat line
//                         if let Some(cap) = JOB_STAT.captures(line_str) {
//                             let (_, [stat_name, samples_str, _, min_str, max_str, sum_str, _]) = cap.extract();

//                             if let (Ok(samples), Ok(min), Ok(max), Ok(sum)) = (
//                                 samples_str.parse::<u64>(),
//                                 min_str.parse::<u64>(),
//                                 max_str.parse::<u64>(),
//                                 sum_str.parse::<u64>(),
//                             ) {
//                                 record_jobstat(
//                                     otel_jobstats,
//                                     stat_name,
//                                     &target,
//                                     &jobid,
//                                     &kind,
//                                     samples,
//                                     min,
//                                     max,
//                                     sum,
//                                 );
//                             }
//                         }
//                     }
//                     Err(TryRecvError::Empty) => {
//                         // No data available, but channel is still open
//                         std::thread::sleep(std::time::Duration::from_millis(10));
//                     }
//                     Err(TryRecvError::Disconnected) => {
//                         // Channel closed, exit loop
//                         break;
//                     }
//                 }
//             }

//             // Wait for the handle to complete
//             if let Err(e) = handle.join() {
//                 tracing::error!("Error joining jobstats stream handle: {:?}", e);
//             }
//         });
//     }
// }

#[cfg(test)]
pub mod tests {
    use const_format::{formatcp, str_repeat};

    use crate::jobstats::jobstats_stream;
    use std::{fs::File, io::BufReader};

    #[tokio::test(flavor = "multi_thread")]
    async fn parse_larger_yaml() {
        let f = File::open("fixtures/jobstats_only/ds86.txt").unwrap();

        let f = BufReader::with_capacity(128 * 1_024, f);

        let (fut, mut rx) = jobstats_stream(f);

        let mut cnt = 0;

        while rx.recv().await.is_some() {
            cnt += 1;
        }

        fut.await.unwrap();

        assert_eq!(cnt, 21_147_876 + 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn parse_large_yaml() {
        let f = File::open("fixtures/jobstats_only/co-vm03.txt").unwrap();

        let f = BufReader::with_capacity(128 * 1_024, f);

        let (fut, mut rx) = jobstats_stream(f);

        let mut cnt = 0;

        while rx.recv().await.is_some() {
            cnt += 1;
        }

        fut.await.unwrap();

        assert_eq!(cnt, 5_310_036 + 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn parse_new_yaml() {
        let f = File::open("fixtures/jobstats_only/2.14.0_162.txt").unwrap();

        let f = BufReader::with_capacity(128 * 1_024, f);

        let (fut, mut rx) = jobstats_stream(f);

        let mut cnt = 0;

        while rx.recv().await.is_some() {
            cnt += 1;
        }

        fut.await.unwrap();

        assert_eq!(cnt, 1_728 + 1);
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

        fut.await.unwrap();

        assert_eq!(
            output.lines().count(),
            (4 + // 4 metrics per read_bytes
             4 + // 4 metrics per write_bytes
             10) // 10 metrics for "getattr" | "setattr" | "punch" | "sync" | "destroy" | "create" | "statfs" | "get_info" | "set_info" | "quotactl"
             * 10
                + 1
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn parse_some_empty() {
        let f = File::open("fixtures/jobstats_only/some_empty.txt").unwrap();

        let f = BufReader::with_capacity(128 * 1_024, f);

        let (fut, mut rx) = jobstats_stream(f);

        let mut cnt = 0;

        while rx.recv().await.is_some() {
            cnt += 1;
        }

        fut.await.unwrap();

        assert_eq!(cnt, 108 + 1);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn parse_2_14_0_164_jobstats() {
        let f = File::open("fixtures/jobstats_only/2.14.0_164.txt").unwrap();

        let f = BufReader::with_capacity(128 * 1_024, f);

        let (fut, mut rx) = jobstats_stream(f);

        let mut output = r#"previous_stat{foo="bar"} 0"#.to_string();

        while let Some(x) = rx.recv().await {
            output.push_str(x.as_str());
        }

        fut.await.unwrap();

        insta::assert_snapshot!(output);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn parse_2_14_0_164_jobstats_otel() {
        let f = File::open("fixtures/jobstats_only/2.14.0_164.txt").unwrap();

        let f = BufReader::with_capacity(128 * 1_024, f);

        let (fut, mut rx) = jobstats_stream(f);

        let mut output = r#"previous_stat{foo="bar"} 0"#.to_string();

        while let Some(x) = rx.recv().await {
            output.push_str(x.as_str());
        }

        fut.await.unwrap();

        insta::assert_snapshot!(output);
    }
}

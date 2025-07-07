// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

#[derive(Debug)]
enum State {
    Empty,
    Target(String),
    TargetJob(String, String),
    TargetJobStats(String, String, Vec<String>),
}

pub mod opentelemetry {
    use crate::{Error, LabelProm, jobstats::State};
    use lustre_collector::TargetVariant;
    use opentelemetry::{
        KeyValue,
        metrics::{Counter, Gauge, Meter},
    };
    use regex::Regex;
    use std::{
        io::BufRead,
        sync::{Arc, LazyLock},
    };
    use tokio::task::JoinHandle;

    #[derive(Debug)]
    pub struct OpenTelemetryMetricsJobstats {
        pub read_samples_total: Counter<u64>,
        pub read_minimum_size_bytes: Gauge<u64>,
        pub read_maximum_size_bytes: Counter<u64>,
        pub read_bytes_total: Counter<u64>,
        pub write_samples_total: Counter<u64>,
        pub write_minimum_size_bytes: Gauge<u64>,
        pub write_maximum_size_bytes: Counter<u64>,
        pub write_bytes_total: Counter<u64>,
        pub stats_total: Counter<u64>,
    }

    impl OpenTelemetryMetricsJobstats {
        pub fn new(meter: &Meter) -> Self {
            OpenTelemetryMetricsJobstats {
                read_samples_total: meter
                    .u64_counter("lustre_job_read_samples_total")
                    .with_description("Total number of reads that have been recorded.")
                    .build(),
                read_minimum_size_bytes: meter
                    .u64_gauge("lustre_job_read_minimum_size_bytes")
                    .with_description("The minimum read size in bytes.")
                    .build(),
                read_maximum_size_bytes: meter
                    .u64_counter("lustre_job_read_maximum_size_bytes")
                    .with_description("The maximum read size in bytes.")
                    .build(),
                read_bytes_total: meter
                    .u64_counter("lustre_job_read_bytes_total")
                    .with_description("The total number of bytes that have been read.")
                    .build(),
                write_samples_total: meter
                    .u64_counter("lustre_job_write_samples_total")
                    .with_description("Total number of writes that have been recorded.")
                    .build(),
                write_minimum_size_bytes: meter
                    .u64_gauge("lustre_job_write_minimum_size_bytes")
                    .with_description("The minimum write size in bytes.")
                    .build(),
                write_maximum_size_bytes: meter
                    .u64_counter("lustre_job_write_maximum_size_bytes")
                    .with_description("The maximum write size in bytes.")
                    .build(),
                write_bytes_total: meter
                    .u64_counter("lustre_job_write_bytes_total")
                    .with_description("The total number of bytes that have been written.")
                    .build(),
                stats_total: meter
                    .u64_counter("lustre_job_stats_total")
                    .with_description(
                        "Number of operations the filesystem has performed, recorded by jobstats.",
                    )
                    .build(),
            }
        }
    }

    pub fn jobstats_stream<R: BufRead + std::marker::Send + 'static>(
        f: R,
        otel_jobstats: Arc<OpenTelemetryMetricsJobstats>,
    ) -> JoinHandle<()> {
        enum LoopInstruction {
            Noop,
            Return,
        }

        fn handle_line(
            otel_jobstats: &OpenTelemetryMetricsJobstats,
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
                    return Ok((state, LoopInstruction::Noop));
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
                    render_stat(otel_jobstats, &target, job, stats)?;

                    state = State::TargetJob(target, line);
                }
                State::TargetJobStats(target, job, stats)
                    if line.starts_with("obdfilter") || line.starts_with("mdt.") =>
                {
                    render_stat(otel_jobstats, &target, job, stats)?;

                    state = State::Target(line);
                }
                x => {
                    tracing::debug!("Unexpected line: {line}, state: {x:?}");
                    return Ok((x, LoopInstruction::Return));
                }
            }

            Ok((state, LoopInstruction::Noop))
        }

        tokio::task::spawn_blocking(move || {
            let mut state = State::Empty;

            for line in f.lines() {
                let r = handle_line(&otel_jobstats, line.map_err(Error::Io), state);

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
                if let Err(e) = render_stat(&otel_jobstats, &target, job, stats) {
                    tracing::debug!("Unexpected error processing jobstats lines: {e}");
                };
            }
        })
    }

    static TARGET: LazyLock<regex::Regex> = LazyLock::new(|| {
        Regex::new(r#"^(obdfilter|mdt)\.([a-zA-Z0-9_-]+)\.job_stats=$"#)
            .expect("A Well-formed regex")
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

    fn render_stat(
        otel_jobstats: &OpenTelemetryMetricsJobstats,
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

            let min = min.parse();
            let max = max.parse();
            let sum = sum.parse();
            let samples = samples.parse();

            let base_labels = &[
                KeyValue::new("operation", stat_name.to_string()),
                KeyValue::new("component", kind.to_prom_label().to_string()),
                KeyValue::new("target", target.to_string()),
                KeyValue::new("jobid", jobid.to_string()),
            ];

            if kind == TargetVariant::Ost {
                match stat_name {
                    "read_bytes" => {
                        if let Ok(samples) = samples {
                            otel_jobstats.read_samples_total.add(samples, base_labels);
                        }
                        if let Ok(min) = min {
                            otel_jobstats
                                .read_minimum_size_bytes
                                .record(min, base_labels);
                        }
                        if let Ok(max) = max {
                            otel_jobstats.read_maximum_size_bytes.add(max, base_labels);
                        }
                        if let Ok(sum) = sum {
                            otel_jobstats.read_bytes_total.add(sum, base_labels);
                        }
                    }
                    "write_bytes" => {
                        if let Ok(samples) = samples {
                            otel_jobstats.write_samples_total.add(samples, base_labels);
                        }
                        if let Ok(min) = min {
                            otel_jobstats
                                .write_minimum_size_bytes
                                .record(min, base_labels);
                        }
                        if let Ok(max) = max {
                            otel_jobstats.write_maximum_size_bytes.add(max, base_labels);
                        }
                        if let Ok(sum) = sum {
                            otel_jobstats.write_bytes_total.add(sum, base_labels);
                        }
                    }
                    "getattr" | "setattr" | "punch" | "sync" | "destroy" | "create" | "statfs"
                    | "get_info" | "set_info" | "quotactl" => {
                        if let Ok(samples) = samples {
                            otel_jobstats.stats_total.add(samples, base_labels);
                        }
                    }
                    _ => {
                        // Unhandled OST jobstats stats
                        tracing::debug!("Unhandled OST jobstats stats: {stat_name}");
                    }
                }
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
                        if let Ok(samples) = samples {
                            otel_jobstats.stats_total.add(samples, base_labels);
                        }
                    }
                    _ => {
                        // Unhandled MDT jobstats stats
                        tracing::debug!("Unhandled MDT jobstats stats: {stat_name}");
                    }
                }
            }
        }

        Ok(())
    }

    // Function to process a jobstats file directly to OpenTelemetry metrics
    pub fn process_jobstats_file<R: BufRead + std::marker::Send + 'static>(
        stream: R,
        otel_jobstats: Arc<OpenTelemetryMetricsJobstats>,
    ) -> JoinHandle<()> {
        jobstats_stream(stream, otel_jobstats)
    }
}

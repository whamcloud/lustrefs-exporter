// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{Error, Family, LabelContainer, LabelProm};
use lustre_collector::TargetVariant;
use prometheus_client::{
    metrics::{counter::Counter, gauge::Gauge},
    registry::Registry,
};
use regex::Regex;
use std::{
    io::BufRead,
    sync::{
        LazyLock,
        atomic::{AtomicU64, Ordering},
    },
};
use tokio::task::JoinHandle;

#[derive(Debug)]
enum State {
    Empty,
    Target(String),
    TargetJob(String, String),
    TargetJobStats(String, String, Vec<String>),
}

#[derive(Debug, Default, Clone)]
pub struct JobstatMetrics {
    read_samples_total: Family<Counter<u64>>,
    read_minimum_size_bytes: Family<Gauge<u64, AtomicU64>>,
    read_maximum_size_bytes: Family<Counter<u64>>,
    read_bytes_total: Family<Counter<u64>>,
    write_samples_total: Family<Counter<u64>>,
    write_minimum_size_bytes: Family<Gauge<u64, AtomicU64>>,
    write_maximum_size_bytes: Family<Counter<u64>>,
    write_bytes_total: Family<Counter<u64>>,
    stats_total: Family<Counter<u64>>,
    target_info: Family<Gauge<u64, AtomicU64>>,
}

// Directly sets a Counter's underlying atomic value. This is required by A7
// (persistent `JobstatMetrics` across scrapes): Lustre reports cumulative
// counts since last reset, so each scrape must overwrite the exported value
// rather than accumulate it.
fn store_counter(counter: &Counter<u64>, value: u64) {
    counter.inner().store(value, Ordering::Relaxed);
}

impl JobstatMetrics {
    pub fn register_metric(&self, registry: &mut Registry) {
        registry.register(
            "lustre_job_read_samples",
            "Total number of reads that have been recorded",
            self.read_samples_total.clone(),
        );

        registry.register(
            "lustre_job_read_minimum_size_bytes",
            "The minimum read size in bytes",
            self.read_minimum_size_bytes.clone(),
        );

        registry.register_without_auto_suffix(
            "lustre_job_read_maximum_size_bytes",
            "The maximum read size in bytes",
            self.read_maximum_size_bytes.clone(),
        );

        registry.register(
            "lustre_job_read_bytes",
            "The total number of bytes that have been read",
            self.read_bytes_total.clone(),
        );

        registry.register(
            "lustre_job_write_samples",
            "Total number of writes that have been recorded",
            self.write_samples_total.clone(),
        );

        registry.register(
            "lustre_job_write_minimum_size_bytes",
            "The minimum write size in bytes",
            self.write_minimum_size_bytes.clone(),
        );

        registry.register_without_auto_suffix(
            "lustre_job_write_maximum_size_bytes",
            "The maximum write size in bytes",
            self.write_maximum_size_bytes.clone(),
        );

        registry.register(
            "lustre_job_write_bytes",
            "The total number of bytes that have been written",
            self.write_bytes_total.clone(),
        );

        registry.register(
            "lustre_job_stats",
            "Number of operations the filesystem has performed, recorded by jobstats",
            self.stats_total.clone(),
        );

        registry.register("target_info", "Target metadata", self.target_info.clone());
    }
}

pub fn jobstats_stream<R: BufRead + std::marker::Send + 'static>(
    f: R,
    mut jobstats: JobstatMetrics,
) -> JoinHandle<JobstatMetrics> {
    enum LoopInstruction {
        Noop,
        Return,
    }

    #[allow(clippy::result_large_err)]
    fn handle_line(
        jobstats: &mut JobstatMetrics,
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
                render_stat(jobstats, &target, job, stats)?;

                state = State::TargetJob(target, line);
            }
            State::TargetJobStats(target, job, stats)
                if line.starts_with("obdfilter") || line.starts_with("mdt.") =>
            {
                render_stat(jobstats, &target, job, stats)?;

                state = State::Target(line);
            }
            x => {
                tracing::debug!("Unexpected line: {line}, state: {x:?}");

                return Ok((x, LoopInstruction::Return));
            }
        }

        Ok((state, LoopInstruction::Noop))
    }

    tokio::spawn(async move {
        let mut state = State::Empty;

        for line in f.lines() {
            let r = handle_line(&mut jobstats, line.map_err(Error::Io), state);

            match r {
                Ok((new_state, LoopInstruction::Noop)) => state = new_state,
                Ok((_, LoopInstruction::Return)) => return jobstats,
                Err(e) => {
                    tracing::debug!("Unexpected error processing jobstats lines: {e}");

                    return jobstats;
                }
            }
        }

        if let State::TargetJobStats(target, job, stats) = state
            && let Err(e) = render_stat(&mut jobstats, &target, job, stats)
        {
            tracing::debug!("Unexpected error processing jobstats lines: {e}");
        };

        jobstats
    })
}

static TARGET: LazyLock<regex::Regex> = LazyLock::new(|| {
    Regex::new(r#"^(obdfilter|mdt)\.([a-zA-Z0-9_-]+)\.job_stats=$"#).expect("A Well-formed regex")
});

static JOB_STAT: LazyLock<regex::Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?x)
            ^\ \ (?<stat>[a-z_]+):\ +\{         # 1. stat name
            \ samples:\ +(?<sample>[0-9]+),     # 2. sample value
            \ unit:\ +[a-z]+,                 # 3. unit value
            \ min:\ +(?<min>[0-9]+),            # 4. min value
            \ max:\ +(?<max>[0-9]+),            # 5. max value
            \ sum:\ +(?<sum>[0-9]+),            # 6. sum value
            \ sumsq:\ +[0-9]                    # 7. sumsq value
    ",
    )
    .expect("A Well-formed regex")
});

#[allow(clippy::result_large_err)]
fn render_stat(
    jobstats: &mut JobstatMetrics,
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

    let base_labels = vec![
        ("component", kind.to_prom_label().to_string()),
        ("jobid", jobid.to_string()),
        ("target", target.to_string()),
    ];

    for stat in stats {
        let cap = JOB_STAT
            .captures(&stat)
            .ok_or_else(|| Error::NoCap("job_stat", stat.to_owned()))?;

        let (_, [stat_name, samples, min, max, sum]) = cap.extract();

        let Ok(samples) = samples.parse::<u64>() else {
            continue;
        };

        let mut labels = base_labels.clone();
        labels.insert(2, ("operation", stat_name.to_string()));

        if kind == TargetVariant::Ost {
            match stat_name {
                "read_bytes" => {
                    if samples == 0 {
                        remove_read_bytes(jobstats, &labels);
                        continue;
                    }
                    store_counter(&jobstats.read_samples_total.get_or_create(&labels), samples);
                    if let Ok(min) = min.parse() {
                        jobstats
                            .read_minimum_size_bytes
                            .get_or_create(&labels)
                            .set(min);
                    }
                    if let Ok(max) = max.parse() {
                        store_counter(
                            &jobstats.read_maximum_size_bytes.get_or_create(&labels),
                            max,
                        );
                    }
                    if let Ok(sum) = sum.parse() {
                        store_counter(&jobstats.read_bytes_total.get_or_create(&labels), sum);
                    }
                }
                "write_bytes" => {
                    if samples == 0 {
                        remove_write_bytes(jobstats, &labels);
                        continue;
                    }
                    store_counter(
                        &jobstats.write_samples_total.get_or_create(&labels),
                        samples,
                    );
                    if let Ok(min) = min.parse() {
                        jobstats
                            .write_minimum_size_bytes
                            .get_or_create(&labels)
                            .set(min);
                    }
                    if let Ok(max) = max.parse() {
                        store_counter(
                            &jobstats.write_maximum_size_bytes.get_or_create(&labels),
                            max,
                        );
                    }
                    if let Ok(sum) = sum.parse() {
                        store_counter(&jobstats.write_bytes_total.get_or_create(&labels), sum);
                    }
                }
                "getattr" | "setattr" | "punch" | "sync" | "destroy" | "create" | "statfs"
                | "get_info" | "set_info" | "quotactl" => {
                    if samples == 0 {
                        jobstats.stats_total.remove(&labels);
                        continue;
                    }
                    store_counter(&jobstats.stats_total.get_or_create(&labels), samples);
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
                    if samples == 0 {
                        jobstats.stats_total.remove(&labels);
                        continue;
                    }
                    store_counter(&jobstats.stats_total.get_or_create(&labels), samples);
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

fn remove_read_bytes(jobstats: &JobstatMetrics, labels: &LabelContainer) {
    jobstats.read_samples_total.remove(labels);
    jobstats.read_minimum_size_bytes.remove(labels);
    jobstats.read_maximum_size_bytes.remove(labels);
    jobstats.read_bytes_total.remove(labels);
}

fn remove_write_bytes(jobstats: &JobstatMetrics, labels: &LabelContainer) {
    jobstats.write_samples_total.remove(labels);
    jobstats.write_minimum_size_bytes.remove(labels);
    jobstats.write_maximum_size_bytes.remove(labels);
    jobstats.write_bytes_total.remove(labels);
}

#[cfg(test)]
pub mod tests {
    use prometheus_client::{encoding::text::encode, registry::Registry};

    use crate::{
        jobstats::{self, JobstatMetrics},
        tests::{
            compare_metrics, get_scrape, historical_snapshot_path, read_metrics_from_snapshot,
        },
    };
    use std::{
        fs::File,
        io::{BufRead, BufReader},
    };

    async fn stream_jobstats<R: BufRead + std::marker::Send + 'static>(f: R) -> String {
        let mut registry = Registry::default();
        let metrics = JobstatMetrics::default();

        let stream = BufReader::with_capacity(128 * 1_024, f);

        let jobstats = jobstats::jobstats_stream(stream, metrics).await.unwrap();

        jobstats.register_metric(&mut registry);

        let mut buffer = String::new();

        encode(&mut buffer, &registry).unwrap();

        buffer
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn parse_larger_yaml() {
        let f = BufReader::new(File::open("fixtures/jobstats_only/ds86.txt").unwrap());

        let buffer = stream_jobstats(f).await;

        // Zero-valued stats are not emitted (see jobstats_zero_values.md);
        // only records with `samples > 0` contribute rows.
        assert_eq!(buffer.lines().count(), 566975);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn parse_large_yaml() {
        let f = BufReader::new(File::open("fixtures/jobstats_only/co-vm03.txt").unwrap());

        let buffer = stream_jobstats(f).await;

        // Zero-valued stats are not emitted (see jobstats_zero_values.md); the
        // exact count depends on how many ops each of the 49167 jobs actually
        // performed.
        assert_eq!(buffer.lines().count(), 196677);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn parse_new_yaml() {
        let f = BufReader::new(File::open("fixtures/jobstats_only/2.14.0_162.txt").unwrap());

        let buffer = stream_jobstats(f).await;

        // Zero-valued stats are not emitted (see jobstats_zero_values.md).
        assert_eq!(buffer.lines().count(), 91);
    }

    fn create_job_template(job_id: &str) -> String {
        format!(
            r#"- job_id:          "{}"
  snapshot_time:   1720516680
  read_bytes:      {{ samples:           0, unit: bytes, min:        0, max:        0, sum:                0, sumsq:                  0 }}
  write_bytes:     {{ samples:          52, unit: bytes, min:     4096, max:   475136, sum:          5468160, sumsq:      1071040692224 }}
  read:            {{ samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }}
  write:           {{ samples:          52, unit: usecs, min:       12, max:    40081, sum:           692342, sumsq:        17432258604 }}
  getattr:         {{ samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }}
  setattr:         {{ samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }}
  punch:           {{ samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }}
  sync:            {{ samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }}
  destroy:         {{ samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }}
  create:          {{ samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }}
  statfs:          {{ samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }}
  get_info:        {{ samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }}
  set_info:        {{ samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }}
  quotactl:        {{ samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }}
  prealloc:        {{ samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }}"#,
            job_id
        )
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn parse_synthetic_yaml() -> Result<(), Box<dyn std::error::Error>> {
        // Make the string static so it lives through the entire test
        let input_10_jobs = format!(
            r#"obdfilter.ds002-OST0000.job_stats=
job_stats:
{}"#,
            (0..10)
                .map(|i| create_job_template(&i.to_string()))
                .collect::<Vec<_>>()
                .join("\n")
        );

        // Convert to bytes and then to cursor to avoid borrowing issues
        let bytes = input_10_jobs.into_bytes();

        let buffer = stream_jobstats(BufReader::with_capacity(
            128 * 1_024,
            std::io::Cursor::new(bytes),
        ))
        .await;

        // Only `write_bytes` has samples > 0 in the template, so only its 4
        // metrics are emitted per job (see jobstats_zero_values.md).
        assert_eq!(
            buffer.lines().count(),
            4 // 4 metrics per write_bytes
                * 10 // 10 jobs
                + 2 * 4 // HELP and TYPE lines for the 4 write_bytes metrics
                + 1 // # EOF
        );

        Ok(())
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn parse_some_empty() {
        let f = BufReader::new(File::open("fixtures/jobstats_only/some_empty.txt").unwrap());

        let buffer = stream_jobstats(f).await;

        // The only non-zero record in the fixture is `quotactl` on a single
        // job, which produces a single `lustre_job_stats_total` line.
        assert_eq!(
            buffer.lines().count(),
            1 // 1 stats_total row for the sole quotactl record
                + 2 // HELP and TYPE lines for lustre_job_stats
                + 1 // # EOF
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn parse_2_14_0_164_jobstats_otel() {
        let f = BufReader::new(File::open("fixtures/jobstats_only/2.14.0_164.txt").unwrap());

        let stats = stream_jobstats(f).await;

        insta::assert_snapshot!(stats);

        let current = get_scrape(stats);

        let previous = read_metrics_from_snapshot(&historical_snapshot_path(
            "lustrefs_exporter__jobstats__tests__parse_2_14_0_164_jobstats.histsnap",
        ));

        compare_metrics(&current, &previous);
    }
}

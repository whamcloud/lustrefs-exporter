// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use std::{
    fs::File,
    io::{BufRead, BufReader},
};

use lustrefs_exporter::jobstats::JobstatMetrics;
use prometheus_client::{encoding::text::encode, registry::Registry};

async fn stream_jobstats<R: BufRead + std::marker::Send + 'static>(f: R) -> Registry {
    let mut registry = Registry::default();
    let otel_jobstats = JobstatMetrics::default();

    let otel_jobstats = otel_jobstats;

    let stream = BufReader::with_capacity(128 * 1_024, f);

    let otel_jobstats = lustrefs_exporter::jobstats::jobstats_stream(stream, otel_jobstats)
        .await
        .unwrap();

    otel_jobstats.register_metric(&mut registry);

    registry
}

#[tokio::test(flavor = "multi_thread")]
async fn parse_larger_yaml() {
    let f = BufReader::new(File::open("fixtures/jobstats_only/ds86.txt").unwrap());

    let registry = stream_jobstats(f).await;

    let mut buffer = String::new();
    encode(&mut buffer, &registry).unwrap();

    assert_eq!(buffer.lines().count(), 3524668);
}

#[tokio::test(flavor = "multi_thread")]
async fn parse_large_yaml() {
    let f = BufReader::new(File::open("fixtures/jobstats_only/co-vm03.txt").unwrap());

    let registry = stream_jobstats(f).await;

    let mut buffer = String::new();
    encode(&mut buffer, &registry).unwrap();

    assert_eq!(
        buffer.lines().count(),
        (4 + // 4 metrics per read_bytes
            4 + // 4 metrics per write_bytes
            10) // 10 metrics for "getattr" | "setattr" | "punch" | "sync" | "destroy" | "create" | "statfs" | "get_info" | "set_info" | "quotactl"
            * 49167 // 49167 jobs
               + 2 * 9 // HELP and TYPE lines
               + 3 // target_info line + HELP and TYPE
               + 1 // # EOF
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn parse_new_yaml() {
    let f = BufReader::new(File::open("fixtures/jobstats_only/2.14.0_162.txt").unwrap());

    let registry = stream_jobstats(f).await;

    let mut buffer = String::new();
    encode(&mut buffer, &registry).unwrap();

    assert_eq!(
        buffer.lines().count(),
        (4 + // 4 metrics per read_bytes
            4 + // 4 metrics per write_bytes
            10) // 10 metrics for "getattr" | "setattr" | "punch" | "sync" | "destroy" | "create" | "statfs" | "get_info" | "set_info" | "quotactl"
            * 16 // 16 jobs
               + 2 * 9 // HELP and TYPE lines
               + 3 // target_info line + HELP and TYPE
               + 1 // # EOF
    );
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

    let mut registry = Registry::default();

    let otel_jobstats = JobstatMetrics::default();

    let otel_jobstats = otel_jobstats;

    let f = BufReader::with_capacity(128 * 1_024, std::io::Cursor::new(bytes));

    let otel_jobstats = lustrefs_exporter::jobstats::jobstats_stream(f, otel_jobstats).await?;

    otel_jobstats.register_metric(&mut registry);

    let mut buffer = String::new();
    encode(&mut buffer, &registry).unwrap();

    assert_eq!(
        buffer.lines().count(),
        (4 + // 4 metrics per read_bytes
             4 + // 4 metrics per write_bytes
             10) // 10 metrics for "getattr" | "setattr" | "punch" | "sync" | "destroy" | "create" | "statfs" | "get_info" | "set_info" | "quotactl"
             * 10 // 10 jobs
                + 2 * 9 // HELP and TYPE lines
                + 3 // target_info line + HELP and TYPE
                + 1 // # EOF
    );

    Ok(())
}

#[tokio::test(flavor = "multi_thread")]
async fn parse_some_empty() {
    let f = BufReader::new(File::open("fixtures/jobstats_only/some_empty.txt").unwrap());

    let registry = stream_jobstats(f).await;

    let mut buffer = String::new();
    encode(&mut buffer, &registry).unwrap();

    assert_eq!(
        buffer.lines().count(),
        (4 + // 4 metrics per read_bytes
            4 + // 4 metrics per write_bytes
            10) // 10 metrics for "getattr" | "setattr" | "punch" | "sync" | "destroy" | "create" | "statfs" | "get_info" | "set_info" | "quotactl"
            * 1 // 10 jobs
               + 2 * 9 // HELP and TYPE lines
               + 3 // target_info line + HELP and TYPE
               + 1 // # EOF
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn parse_2_14_0_164_jobstats_otel() {
    let f = BufReader::new(File::open("fixtures/jobstats_only/2.14.0_164.txt").unwrap());

    let registry = stream_jobstats(f).await;

    let mut buffer = String::new();
    encode(&mut buffer, &registry).unwrap();

    insta::assert_snapshot!(buffer);
}

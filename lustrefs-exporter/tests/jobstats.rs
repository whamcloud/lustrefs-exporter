use lustrefs_exporter::{
    init_opentelemetry, jobstats, jobstats::opentelemetry::OpenTelemetryMetricsJobstats,
};
use opentelemetry::metrics::MeterProvider;
use prometheus::{Encoder as _, Registry, TextEncoder};
use std::{fs::File, io::BufReader, sync::Arc};

#[tokio::test(flavor = "multi_thread")]
async fn parse_larger_yaml() {
    let f = File::open("fixtures/jobstats_only/ds86.txt").unwrap();

    let f = BufReader::with_capacity(128 * 1_024, f);

    // Set up OpenTelemetry metrics
    let (provider, registry) = init_opentelemetry().unwrap();

    let meter = provider.meter("test");
    let otel_jobstats = Arc::new(OpenTelemetryMetricsJobstats::new(&meter));

    let handle = jobstats::opentelemetry::jobstats_stream(f, otel_jobstats.clone());

    // Allow time for processing
    handle.await.unwrap();

    let cnt = get_output(&registry).lines().count();

    assert_eq!(cnt, 3524667);
}

#[tokio::test(flavor = "multi_thread")]
async fn parse_large_yaml() {
    let f = File::open("fixtures/jobstats_only/co-vm03.txt").unwrap();

    let f = BufReader::with_capacity(128 * 1_024, f);

    // Set up OpenTelemetry metrics
    let (provider, registry) = init_opentelemetry().unwrap();
    let meter = provider.meter("test");
    let otel_jobstats = Arc::new(OpenTelemetryMetricsJobstats::new(&meter));

    let handle = jobstats::opentelemetry::jobstats_stream(f, otel_jobstats.clone());

    // Allow time for processing
    handle.await.unwrap();

    let cnt = get_output(&registry).lines().count();

    assert_eq!(
        cnt,
        (4 + // 4 metrics per read_bytes
            4 + // 4 metrics per write_bytes
            10) // 10 metrics for "getattr" | "setattr" | "punch" | "sync" | "destroy" | "create" | "statfs" | "get_info" | "set_info" | "quotactl"
            * 49167 // 49167 jobs
               + 2 * 9 // HELP and TYPE lines
               + 3 // target_info line + HELP and TYPE
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn parse_new_yaml() {
    let f = File::open("fixtures/jobstats_only/2.14.0_162.txt").unwrap();

    let f = BufReader::with_capacity(128 * 1_024, f);

    // Set up OpenTelemetry metrics
    let (provider, registry) = init_opentelemetry().unwrap();
    let meter = provider.meter("test");
    let otel_jobstats = Arc::new(OpenTelemetryMetricsJobstats::new(&meter));

    let handle = jobstats::opentelemetry::jobstats_stream(f, otel_jobstats.clone());

    // Allow time for processing
    handle.await.unwrap();

    let cnt = get_output(&registry).lines().count();

    assert_eq!(
        cnt,
        (4 + // 4 metrics per read_bytes
            4 + // 4 metrics per write_bytes
            10) // 10 metrics for "getattr" | "setattr" | "punch" | "sync" | "destroy" | "create" | "statfs" | "get_info" | "set_info" | "quotactl"
            * 16 // 16 jobs
               + 2 * 9 // HELP and TYPE lines
               + 3 // target_info line + HELP and TYPE
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
async fn parse_synthetic_yaml() {
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
    println!("{input_10_jobs}");

    // Convert to bytes and then to cursor to avoid borrowing issues
    let bytes = input_10_jobs.into_bytes();
    let f = BufReader::with_capacity(128 * 1_024, std::io::Cursor::new(bytes));

    // Set up OpenTelemetry metrics
    let (provider, registry) = init_opentelemetry().unwrap();
    let meter = provider.meter("test");
    let otel_jobstats = Arc::new(OpenTelemetryMetricsJobstats::new(&meter));

    let handle = jobstats::opentelemetry::jobstats_stream(f, otel_jobstats.clone());

    // Allow time for processing
    handle.await.unwrap();

    assert_eq!(
        get_output(&registry).lines().count(),
        (4 + // 4 metrics per read_bytes
             4 + // 4 metrics per write_bytes
             10) // 10 metrics for "getattr" | "setattr" | "punch" | "sync" | "destroy" | "create" | "statfs" | "get_info" | "set_info" | "quotactl"
             * 10 // 10 jobs
                + 2 * 9 // HELP and TYPE lines
                + 3 // target_info line + HELP and TYPE
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn parse_some_empty() {
    let f = File::open("fixtures/jobstats_only/some_empty.txt").unwrap();

    let f = BufReader::with_capacity(128 * 1_024, f);

    // Set up OpenTelemetry metrics
    let (provider, registry) = init_opentelemetry().unwrap();
    let meter = provider.meter("test");
    let otel_jobstats = Arc::new(OpenTelemetryMetricsJobstats::new(&meter));

    let handle = jobstats::opentelemetry::jobstats_stream(f, otel_jobstats.clone());

    // Allow time for processing
    handle.await.unwrap();

    let cnt = get_output(&registry).lines().count();

    assert_eq!(
        cnt,
        (4 + // 4 metrics per read_bytes
            4 + // 4 metrics per write_bytes
            10) // 10 metrics for "getattr" | "setattr" | "punch" | "sync" | "destroy" | "create" | "statfs" | "get_info" | "set_info" | "quotactl"
            * 1 // 10 jobs
               + 2 * 9 // HELP and TYPE lines
               + 3 // target_info line + HELP and TYPE
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn parse_2_14_0_164_jobstats_otel() {
    let f = File::open("fixtures/jobstats_only/2.14.0_164.txt").unwrap();

    let f = BufReader::with_capacity(128 * 1_024, f);

    // Set up OpenTelemetry metrics
    let (provider, registry) = init_opentelemetry().unwrap();
    let meter = provider.meter("test");
    let otel_jobstats = Arc::new(OpenTelemetryMetricsJobstats::new(&meter));

    let handle = jobstats::opentelemetry::jobstats_stream(f, otel_jobstats.clone());

    // Allow time for processing
    handle.await.unwrap();

    insta::assert_snapshot!(get_output(&registry));
}

fn get_output(registry: &Registry) -> String {
    let encoder = TextEncoder::new();
    let mut output = Vec::new();
    encoder.encode(&registry.gather(), &mut output).unwrap();
    String::from_utf8(output).unwrap()
}

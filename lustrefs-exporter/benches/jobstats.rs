use const_format::{formatcp, str_repeat};
use criterion::{Criterion, criterion_group, criterion_main};
use lustrefs_exporter::jobstats::opentelemetry::OpenTelemetryMetricsJobstats;
use opentelemetry::metrics::MeterProvider;
use opentelemetry_sdk::metrics::SdkMeterProvider;
use prometheus::{Encoder as _, Registry, TextEncoder};
use std::{hint, io::BufReader, sync::Arc};

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

#[allow(long_running_const_eval)]
const INPUT_100_JOBS: &str = formatcp!(
    r#"obdfilter.ds002-OST0000.job_stats=
job_stats:{}"#,
    str_repeat!(JOBSTAT_JOB, 100)
);

#[allow(long_running_const_eval)]
const INPUT_1000_JOBS: &str = formatcp!(
    r#"obdfilter.ds002-OST0000.job_stats=
job_stats:{}"#,
    str_repeat!(JOBSTAT_JOB, 1000)
);

async fn parse_synthetic_yaml_otel(input: &'static str) {
    // Set up OpenTelemetry metrics
    let registry = Registry::new();
    let exporter = opentelemetry_prometheus::exporter()
        .with_registry(registry.clone())
        .build()
        .unwrap();

    let provider = SdkMeterProvider::builder().with_reader(exporter).build();

    let meter = provider.meter("test");

    let otel_jobstats = Arc::new(OpenTelemetryMetricsJobstats::new(&meter));

    let f = BufReader::with_capacity(128 * 1_024, input.as_bytes());

    let handle =
        lustrefs_exporter::jobstats::opentelemetry::jobstats_stream(f, otel_jobstats.clone());

    handle.await.unwrap();

    // Encode metrics to string
    let encoder = TextEncoder::new();
    let metric_families = registry.gather();
    let mut output = Vec::new();
    encoder.encode(&metric_families, &mut output).unwrap();
}

fn criterion_benchmark_fast(c: &mut Criterion) {
    c.bench_function("jobstats otel 100", |b| {
        b.to_async(tokio::runtime::Builder::new_multi_thread().build().unwrap())
            .iter(|| hint::black_box(parse_synthetic_yaml_otel(INPUT_100_JOBS)))
    });
    c.bench_function("jobstats otel 1000", |b| {
        b.to_async(tokio::runtime::Builder::new_multi_thread().build().unwrap())
            .iter(|| hint::black_box(parse_synthetic_yaml_otel(INPUT_1000_JOBS)))
    });
}
criterion_group! {
    name = benches;
    config = Criterion::default();
    targets = criterion_benchmark_fast
}
criterion_main!(benches);

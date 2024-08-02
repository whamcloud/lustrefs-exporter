use criterion::{black_box, criterion_group, criterion_main, Criterion};
use lustre_collector::{parse_lctl_output, Record};

fn test_data(repeat: usize) -> String {
    let job = r#"
    - job_id:          "SLURM_JOB_machine184_74186:0:ma"
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
    let lctl_output = r#"obdfilter.ds002-OST0000.job_stats=
job_stats:"#;

    let input = format!("{lctl_output}{}", job.to_string().repeat(repeat));

    input
}

fn parse_jobstats(repeat: usize) -> Vec<Record> {
    let input = test_data(repeat);
    parse_lctl_output(input.as_bytes()).unwrap()
}

fn criterion_benchmark_fast(c: &mut Criterion) {
    c.bench_function("jobstats 100", |b| {
        b.iter(|| parse_jobstats(black_box(100)))
    });
    c.bench_function("jobstats 1000", |b| {
        b.iter(|| parse_jobstats(black_box(1000)))
    });
}
criterion_group! {
    name = benches;
    config = Criterion::default();
    targets = criterion_benchmark_fast
}
criterion_main!(benches);

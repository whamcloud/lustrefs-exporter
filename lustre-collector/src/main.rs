// Copyright (c) 2021 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use clap::{value_parser, Arg, ValueEnum};
use combine::{
    attempt, eof, many1, one_of, parser::{
        char::{alpha_num, newline},
        range::{range, recognize, take_fn, take_until_range, TakeRange},
        repeat::take_until,
    }, ParseError, Parser, RangeStream, Stream
};
use lustre_collector::{
    error::LustreCollectorError, mgs::mgs_fs_parser, parse_lctl_output, parse_lnetctl_output,
    parse_lnetctl_stats, parse_mgs_fs_output, parse_recovery_status_output, parser,
    recovery_status_parser, types::Record, JobStatsMdt, JobStatsOst, Target,
};
use std::{
    fmt, panic,
    process::{Command, ExitCode},
    str::{self, FromStr},
    thread,
};
use tracing::debug;

#[derive(ValueEnum, PartialEq, Debug, Clone, Copy)]
enum Format {
    Json,
    Yaml,
}

impl FromStr for Format {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().trim() {
            "json" => Ok(Format::Json),
            "yaml" => Ok(Format::Yaml),
            _ => Err(format!("Could not convert {s} to format type")),
        }
    }
}

impl fmt::Display for Format {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json => write!(f, "json"),
            Self::Yaml => write!(f, "yaml"),
        }
    }
}

fn get_lctl_output() -> Result<Vec<u8>, LustreCollectorError> {
    let lctl_params = parser::params();

    debug!(lctl_params = lctl_params.join(" "));

    let r = Command::new("lctl")
        .arg("get_param")
        .args(lctl_params)
        .output()?;

    Ok(r.stdout)
}

fn get_lctl_mgs_fs_output() -> Result<Vec<u8>, LustreCollectorError> {
    let r = Command::new("lctl")
        .arg("get_param")
        .arg("-N")
        .args(mgs_fs_parser::params())
        .output()?;

    Ok(r.stdout)
}

fn get_recovery_status_output() -> Result<Vec<u8>, LustreCollectorError> {
    let r = Command::new("lctl")
        .arg("get_param")
        .args(recovery_status_parser::params())
        .output()?;

    Ok(r.stdout)
}

fn get_lnetctl_stats_output() -> Result<Vec<u8>, LustreCollectorError> {
    let r = Command::new("lnetctl").arg("stats").arg("show").output()?;

    Ok(r.stdout)
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");

            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<(), LustreCollectorError> {
    tracing_subscriber::fmt::init();

    let matches = clap::Command::new(env!("CARGO_PKG_NAME"))
        .version(env!("CARGO_PKG_VERSION"))
        .author("Whamcloud")
        .about("Grabs various Lustre statistics for display in JSON or YAML")
        .arg(
            Arg::new("format")
                .short('f')
                .long("format")
                .value_parser(value_parser!(Format))
                .default_value("json")
                .help("Sets the output formatting"),
        )
        .get_matches();

    let format = matches
        .get_one::<Format>("format")
        .expect("Required argument `format` missing");

    // let handle = thread::spawn(move || -> Result<Vec<Record>, LustreCollectorError> {
    //     let lctl_output = get_lctl_output()?;

    //     let lctl_record = parse_lctl_output(&lctl_output)?;

    //     Ok(lctl_record)
    // });

    // let mgs_fs_handle = thread::spawn(move || -> Result<Vec<Record>, LustreCollectorError> {
    //     let lctl_output = get_lctl_mgs_fs_output()?;
    //     let lctl_record = parse_mgs_fs_output(&lctl_output)?;

    //     Ok(lctl_record)
    // });

    // let lnetctl_stats_handle =
    //     thread::spawn(move || -> Result<Vec<Record>, LustreCollectorError> {
    //         let lnetctl_stats_output = get_lnetctl_stats_output()?;
    //         let lnetctl_stats_record = parse_lnetctl_stats(str::from_utf8(&lnetctl_stats_output)?)?;

    //         Ok(lnetctl_stats_record)
    //     });

    // let recovery_status_handle =
    //     thread::spawn(move || -> Result<Vec<Record>, LustreCollectorError> {
    //         let recovery_status_output = get_recovery_status_output()?;
    //         let recovery_statuses = parse_recovery_status_output(&recovery_status_output)?;

    //         Ok(recovery_statuses)
    //     });

    // let lnetctl_net_show_output = Command::new("lnetctl")
    //     .args(["net", "show", "-v", "4"])
    //     .output()
    //     .expect("failed to get lnetctl stats");

    // let lnetctl_net_show_stats = str::from_utf8(&lnetctl_net_show_output.stdout)
    //     .expect("while converting 'lnetctl net show -v 4' stdout from utf8");

    // let mut lnet_record = parse_lnetctl_output(lnetctl_net_show_stats)
    //     .expect("while parsing 'lnetctl net show -v 4' stats");

    // let mut lctl_record = match handle.join() {
    //     Ok(r) => r?,
    //     Err(e) => panic::resume_unwind(e),
    // };

    // let mut mgs_fs_record = match mgs_fs_handle.join() {
    //     Ok(r) => r.unwrap_or_default(),
    //     Err(e) => panic::resume_unwind(e),
    // };

    // let mut recovery_status_records = match recovery_status_handle.join() {
    //     Ok(r) => r.unwrap_or_default(),
    //     Err(e) => panic::resume_unwind(e),
    // };

    // let mut lnetctl_stats_record = match lnetctl_stats_handle.join() {
    //     Ok(r) => r.unwrap_or_default(),
    //     Err(e) => panic::resume_unwind(e),
    // };

    // lctl_record.append(&mut lnet_record);
    // lctl_record.append(&mut mgs_fs_record);
    // lctl_record.append(&mut recovery_status_records);
    // lctl_record.append(&mut lnetctl_stats_record);

    // let x = match format {
    //     Format::Json => serde_json::to_string(&lctl_record)?,
    //     Format::Yaml => serde_yaml::to_string(&lctl_record)?,
    // };

    // println!("{x}");
    // let lctl_output = std::fs::read("/root/lustrefs-exporter/lustre-collector/src/fixtures/valid/lustre-2.14.0_ddn125/ds86.txt")?;

    use std::time::Instant;

    let repeat = 100000;

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

    let now = Instant::now();

    let file = std::fs::read_to_string("/root/lustrefs-exporter/lustre-collector/src/fixtures/valid/lustre-2.14.0_ddn125/jobstats_only.txt").unwrap();

    let elapsed = now.elapsed();
    println!("'read_to_string' took: {:.2?}", elapsed);

    if false {
        let now = Instant::now();

        let lctl_record = parse_lctl_output(input.as_bytes())?;

        let elapsed = now.elapsed();
        println!("'parse_lctl_output' took: {:.2?}", elapsed);
        let val = lctl_record.iter().next().unwrap();
        match val {
            Record::Host(_) => todo!(),
            Record::LNetStat(_) => todo!(),
            Record::LustreService(_) => todo!(),
            Record::Node(_) => todo!(),
            Record::Target(t) => match t {
                lustre_collector::TargetStats::JobStatsOst(x) => {
                    let job = x.value.as_ref().unwrap();
                    println!("'parse_lctl_output': Size of jobstat: {}", job.len());
                }
                lustre_collector::TargetStats::Stats(_) => todo!(),
                lustre_collector::TargetStats::BrwStats(_) => todo!(),
                lustre_collector::TargetStats::JobStatsMdt(_) => todo!(),
                lustre_collector::TargetStats::FilesFree(_) => todo!(),
                lustre_collector::TargetStats::FilesTotal(_) => todo!(),
                lustre_collector::TargetStats::FsType(_) => todo!(),
                lustre_collector::TargetStats::KBytesAvail(_) => todo!(),
                lustre_collector::TargetStats::KBytesFree(_) => todo!(),
                lustre_collector::TargetStats::KBytesTotal(_) => todo!(),
                lustre_collector::TargetStats::NumExports(_) => todo!(),
                lustre_collector::TargetStats::TotDirty(_) => todo!(),
                lustre_collector::TargetStats::TotGranted(_) => todo!(),
                lustre_collector::TargetStats::TotPending(_) => todo!(),
                lustre_collector::TargetStats::ContendedLocks(_) => todo!(),
                lustre_collector::TargetStats::ContentionSeconds(_) => todo!(),
                lustre_collector::TargetStats::ConnectedClients(_) => todo!(),
                lustre_collector::TargetStats::CtimeAgeLimit(_) => todo!(),
                lustre_collector::TargetStats::EarlyLockCancel(_) => todo!(),
                lustre_collector::TargetStats::FsNames(_) => todo!(),
                lustre_collector::TargetStats::LockCount(_) => todo!(),
                lustre_collector::TargetStats::LockTimeouts(_) => todo!(),
                lustre_collector::TargetStats::LockUnusedCount(_) => todo!(),
                lustre_collector::TargetStats::LruMaxAge(_) => todo!(),
                lustre_collector::TargetStats::LruSize(_) => todo!(),
                lustre_collector::TargetStats::MaxNolockBytes(_) => todo!(),
                lustre_collector::TargetStats::MaxParallelAst(_) => todo!(),
                lustre_collector::TargetStats::ResourceCount(_) => todo!(),
                lustre_collector::TargetStats::ThreadsMin(_) => todo!(),
                lustre_collector::TargetStats::ThreadsMax(_) => todo!(),
                lustre_collector::TargetStats::ThreadsStarted(_) => todo!(),
                lustre_collector::TargetStats::RecoveryStatus(_) => todo!(),
                lustre_collector::TargetStats::Oss(_) => todo!(),
                lustre_collector::TargetStats::RecoveryConnectedClients(_) => todo!(),
                lustre_collector::TargetStats::RecoveryCompletedClients(_) => todo!(),
                lustre_collector::TargetStats::RecoveryEvictedClients(_) => todo!(),
                lustre_collector::TargetStats::Llite(_) => todo!(),
                lustre_collector::TargetStats::ExportStats(_) => todo!(),
                lustre_collector::TargetStats::Mds(_) => todo!(),
                lustre_collector::TargetStats::Changelog(_) => todo!(),
                lustre_collector::TargetStats::QuotaStats(_) => todo!(),
                lustre_collector::TargetStats::QuotaStatsOsd(_) => todo!(),
            },
        }
        // println!("'lctl_record' len is {}");
        // }
    }

    {
        /// Parses a target name
        pub(crate) fn target<I>() -> impl Parser<I, Output = Target>
        where
            I: Stream<Token = char>,
            I::Error: ParseError<I::Token, I::Range, I::Position>,
        {
            many1(alpha_num().or(one_of("_-".chars()))).map(Target)
        }

        let input = format!("{lctl_output}{}ENDCHAR", job.to_string().repeat(repeat));

        let now = Instant::now();
        let mut parser = (
            range("obdfilter"),
            range("."),
            recognize(target()),
            range("."),
            range("job_stats=\n"),
            take_until_range("ENDCHAR"),
        )
            .map(|(x, y, z, w, v, t): (&str, _, _, _, _, &str)| {
                // println!("{x} {y} {z} {w} {v}");
                // println!("{:#?}", t);

                let x = serde_yaml::from_str(&t)
                    .map(|x: JobStatsOst| x.job_stats)
                    .unwrap();

                x
            });

        let res = parser.parse(input.as_str());

        let elapsed = now.elapsed();
        println!("'range' took: {:.2?}", elapsed);

        match res {
            Ok((v, x)) => {
                match v {
                    Some(x) => println!("'range': Size of jobstats: {}", x.len()),
                    None => println!("'range': No jobs were found"),
                }
            },
            Err(e) => println!("{:#}", e),
        };
    }

    Ok(())
}

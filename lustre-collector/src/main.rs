// Copyright (c) 2021 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use clap::{Arg, ValueEnum, value_parser};
use lustre_collector::{
    error::LustreCollectorError, mgs::mgs_fs_parser, parse_lctl_output, parse_lnetctl_output,
    parse_lnetctl_stats, parse_mgs_fs_output, parse_recovery_status_output, parser,
    recovery_status_parser, types::Record,
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

    let handle = thread::spawn(move || -> Result<Vec<Record>, LustreCollectorError> {
        let lctl_output = get_lctl_output()?;

        let lctl_record = parse_lctl_output(&lctl_output)?;

        Ok(lctl_record)
    });

    let mgs_fs_handle = thread::spawn(move || -> Result<Vec<Record>, LustreCollectorError> {
        let lctl_output = get_lctl_mgs_fs_output()?;
        let lctl_record = parse_mgs_fs_output(&lctl_output)?;

        Ok(lctl_record)
    });

    let lnetctl_stats_handle =
        thread::spawn(move || -> Result<Vec<Record>, LustreCollectorError> {
            let lnetctl_stats_output = get_lnetctl_stats_output()?;
            let lnetctl_stats_record = parse_lnetctl_stats(str::from_utf8(&lnetctl_stats_output)?)?;

            Ok(lnetctl_stats_record)
        });

    let recovery_status_handle =
        thread::spawn(move || -> Result<Vec<Record>, LustreCollectorError> {
            let recovery_status_output = get_recovery_status_output()?;
            let recovery_statuses = parse_recovery_status_output(&recovery_status_output)?;

            Ok(recovery_statuses)
        });

    let lnetctl_net_show_output = Command::new("lnetctl")
        .args(["net", "show", "-v", "4"])
        .output()
        .expect("failed to get lnetctl stats");

    let lnetctl_net_show_stats = str::from_utf8(&lnetctl_net_show_output.stdout)
        .expect("while converting 'lnetctl net show -v 4' stdout from utf8");

    let mut lnet_record = parse_lnetctl_output(lnetctl_net_show_stats)
        .expect("while parsing 'lnetctl net show -v 4' stats");

    let mut lctl_record = match handle.join() {
        Ok(r) => r?,
        Err(e) => panic::resume_unwind(e),
    };

    let mut mgs_fs_record = match mgs_fs_handle.join() {
        Ok(r) => r.unwrap_or_default(),
        Err(e) => panic::resume_unwind(e),
    };

    let mut recovery_status_records = match recovery_status_handle.join() {
        Ok(r) => r.unwrap_or_default(),
        Err(e) => panic::resume_unwind(e),
    };

    let mut lnetctl_stats_record = match lnetctl_stats_handle.join() {
        Ok(r) => r.unwrap_or_default(),
        Err(e) => panic::resume_unwind(e),
    };

    lctl_record.append(&mut lnet_record);
    lctl_record.append(&mut mgs_fs_record);
    lctl_record.append(&mut recovery_status_records);
    lctl_record.append(&mut lnetctl_stats_record);

    let x = match format {
        Format::Json => serde_json::to_string(&lctl_record)?,
        Format::Yaml => serde_yaml::to_string(&lctl_record)?,
    };

    println!("{x}");

    Ok(())
}

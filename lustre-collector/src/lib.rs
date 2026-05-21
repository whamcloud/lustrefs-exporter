// Copyright (c) 2021 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

mod base_parsers;
pub(crate) mod brw_stats_parser;
pub mod error;
pub(crate) mod exports_parser;
pub(crate) mod io_latency_stats_parser;
pub(crate) mod ldlm;
pub(crate) mod llite;
mod lnetctl_parser;
mod mdd_parser;
mod mds;
pub mod mgs;
mod node_stats_parsers;
mod nodemap;
mod osd_parser;
mod oss;
pub mod parser;
pub mod quota;
pub mod recovery_status_parser;
mod stats_parser;
mod time;
mod top_level_parser;
pub mod types;

pub use crate::error::LustreCollectorError;
use combine::{EasyParser, Parser, parser::token::satisfy};
pub use lnetctl_parser::{parse as parse_lnetctl_output, parse_lnetctl_stats};
pub use node_stats_parsers::{parse_cpustats_output, parse_meminfo_output};
use std::{io, str};
pub use types::*;

/// Normalize lctl output to valid YAML format
/// - If a line ends with '=', replace '=' with ':'
/// - For all other non-empty lines, prepend a space
fn normalize_lctl_output<Input>() -> impl Parser<Input, Output = String>
where
    Input: combine::Stream<Token = char>,
    Input::Error: combine::ParseError<Input::Token, Input::Range, Input::Position>,
{
    use combine::{
        attempt, choice, many, many1,
        parser::char::{char, newline},
    };

    let normalize_line = choice((
        // Line ending with '=' -> replace with ':'
        attempt(
            (many1(satisfy(|c| c != '\n' && c != '=')), char('='))
                .map(|(s, _): (String, _)| format!("{}:", s)),
        ),
        // Other non-empty line -> prepend a space
        many1(satisfy(|c| c != '\n')).map(|s: String| format!(" {}", s)),
    ));

    many(normalize_line.skip(newline())).map(|lines: Vec<String>| lines.join("\n"))
}

pub fn parse_osc_state_output(
    osc_state_output: &[u8],
) -> Result<Vec<Record>, LustreCollectorError> {
    let osc_state_str = str::from_utf8(osc_state_output)?;

    // Preprocess the output using combine parser to convert lctl format to valid YAML:
    // - If a line ends with '=', replace '=' with ':'
    // - For non-empty lines that don't end with '=', add a space at the beginning
    let (processed_str, _) = normalize_lctl_output()
        .easy_parse(osc_state_str)
        .map_err(LustreCollectorError::from)?;

    let osc_states: OscStates = serde_yaml::from_str(&processed_str)?;

    // Convert each OSC state to a ControllerStats record
    let records: Vec<Record> = osc_states
        .into_iter()
        .map(|(key, value)| {
            // Remove "osc." prefix and ".state" suffix from keys
            let cleaned_key = key
                .strip_prefix("osc.")
                .unwrap_or(&key)
                .strip_suffix(".state")
                .unwrap_or(&key)
                .to_string();

            Record::Controller(ControllerStats::OscState(ControllerStat {
                kind: ControllerVariant::Osc,
                param: Param("state".to_string()),
                controller: Controller(cleaned_key),
                value,
            }))
        })
        .collect();

    Ok(records)
}

fn check_output(
    records: Vec<Record>,
    state: &str,
    params: &str,
) -> Result<Vec<Record>, LustreCollectorError> {
    if !state.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Content left in input buffer. Please run and supply to support: `lctl get_param {params}`"),
        )
        .into());
    }

    Ok(records)
}

/// Must be called with output of `lctl get_params` for all params returned from `parser::parse()`
pub fn parse_lctl_output(lctl_output: &[u8]) -> Result<Vec<Record>, LustreCollectorError> {
    let lctl_stats = str::from_utf8(lctl_output)?;

    let (lctl_record, state) = parser::parse()
        .easy_parse(lctl_stats)
        .map_err(|err| err.map_position(|p| p.translate_position(lctl_stats)))?;

    let params = parser::params().join(" ");

    check_output(lctl_record, state, &params)
}

pub fn parse_mgs_fs_output(mgs_fs_output: &[u8]) -> Result<Vec<Record>, LustreCollectorError> {
    let mgs_fs = str::from_utf8(mgs_fs_output)?;

    let (mgs_fs_record, state) = mgs::mgs_fs_parser::parse()
        .easy_parse(mgs_fs)
        .map_err(|err| err.map_position(|p| p.translate_position(mgs_fs)))?;

    let params = mgs::mgs_fs_parser::params().join(" ");

    check_output(mgs_fs_record, state, &params)
}

pub fn parse_recovery_status_output(
    recovery_status_output: &[u8],
) -> Result<Vec<Record>, LustreCollectorError> {
    let recovery_status = str::from_utf8(recovery_status_output)?;
    let recovery_status = recovery_status.trim();

    let (recovery_statuses, state) = parser::parse()
        .easy_parse(recovery_status)
        .map_err(|err| err.map_position(|p| p.translate_position(recovery_status)))?;

    let params = recovery_status_parser::params().join(" ");

    check_output(recovery_statuses, state, &params)
}

#[cfg(test)]
mod tests {
    use crate::{parse_lctl_output, parse_mgs_fs_output, parse_recovery_status_output};

    #[test]
    fn ex8761_job_stats() {
        let xs = include_bytes!("./fixtures/valid/ex8761-lctl.txt");
        let expected = parse_lctl_output(xs).unwrap();

        insta::assert_debug_snapshot!(expected);
    }

    #[test]
    fn test_parse_recovery_status_output() {
        let xs = include_bytes!("./fixtures/recovery-multiple.txt");
        let expected = parse_recovery_status_output(xs).unwrap();

        insta::assert_debug_snapshot!(expected);
    }

    #[test]
    fn test_parse_mgs_fs_output() {
        let xs = include_bytes!("./fixtures/mgs-fs.txt");
        let expected = parse_mgs_fs_output(xs).unwrap();

        insta::assert_debug_snapshot!(expected);
    }

    #[test]
    fn es_6_2_0_job_stats_unhealthy() {
        let xs = include_bytes!("./fixtures/valid/params-6.2.0-r9.txt");
        let expected = parse_lctl_output(xs).unwrap();

        insta::assert_debug_snapshot!(expected);
    }

    #[test]
    fn params() {
        let xs = super::parser::params();

        insta::assert_snapshot!(xs.join(" "));
    }
}

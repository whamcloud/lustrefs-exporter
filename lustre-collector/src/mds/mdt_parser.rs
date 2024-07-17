// Copyright (c) 2024 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    base_parsers::{digits, param, param_period, period, target},
    exports_parser::exports_stats,
    mds::job_stats,
    oss::obdfilter_parser::{EXPORTS, EXPORTS_PARAMS},
    stats_parser::stats,
    types::{JobStatMdt, Param, Record, Stat, Target, TargetStat, TargetStats, TargetVariant},
    ExportStats,
};
use combine::{
    attempt, choice,
    error::ParseError,
    parser::char::{newline, string},
    stream::Stream,
    Parser, RangeStream,
};

pub(crate) const JOB_STATS: &str = "job_stats";
pub(crate) const STATS: &str = "md_stats";
pub(crate) const NUM_EXPORTS: &str = "num_exports";

enum MdtStat {
    JobStats(Option<Vec<JobStatMdt>>),
    Stats(Vec<Stat>),
    NumExports(u64),
    ExportStats(Vec<ExportStats>),
}

fn mdt_stat<'a, I>() -> impl Parser<I, Output = (Param, MdtStat)> + 'a
where
    I: RangeStream<Token = char, Range = &'a str> + 'a,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    choice((
        (
            param(NUM_EXPORTS),
            digits().skip(newline()).map(MdtStat::NumExports),
        ),
        (param(STATS), stats().map(MdtStat::Stats)).message("while parsing mdt_stat"),
        (param(JOB_STATS), job_stats::parse().map(MdtStat::JobStats))
            .message("while parsing job_stats"),
        (
            param_period(EXPORTS),
            exports_stats().map(MdtStat::ExportStats),
        ),
    ))
}

pub(crate) fn params() -> Vec<String> {
    [
        format!("mdt.*.{JOB_STATS}"),
        format!("mdt.*.{STATS}"),
        format!("mdt.*MDT*.{NUM_EXPORTS}"),
        format!("mdt.*MDT*.{EXPORTS_PARAMS}"),
    ]
    .into_iter()
    .collect()
}

pub(crate) fn params_no_jobstats() -> Vec<String> {
    [
        format!("mdt.*.{STATS}"),
        format!("mdt.*MDT*.{NUM_EXPORTS}"),
        format!("mdt.*MDT*.{EXPORTS_PARAMS}"),
    ]
    .into_iter()
    .collect()
}

pub(crate) fn params_jobstats_only() -> Vec<String> {
    vec![format!("mdt.*.{JOB_STATS}")]
}

fn target_name<I>() -> impl Parser<I, Output = Target>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        attempt(string("mdt")).skip(period()),
        target().skip(period()),
    )
        .map(|(_, x)| x)
        .message("while parsing target_name")
}

pub(crate) fn parse<'a, I>() -> impl Parser<I, Output = Record> + 'a
where
    I: RangeStream<Token = char, Range = &'a str> + 'a,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (target_name(), mdt_stat())
        .map(|(target, (param, value))| match value {
            MdtStat::JobStats(value) => TargetStats::JobStatsMdt(TargetStat {
                kind: TargetVariant::Mdt,
                target,
                param,
                value,
            }),
            MdtStat::Stats(value) => TargetStats::Stats(TargetStat {
                kind: TargetVariant::Mdt,
                target,
                param,
                value,
            }),
            MdtStat::NumExports(value) => TargetStats::NumExports(TargetStat {
                kind: TargetVariant::Mdt,
                target,
                param,
                value,
            }),
            MdtStat::ExportStats(value) => TargetStats::ExportStats(TargetStat {
                kind: TargetVariant::Mdt,
                target,
                param,
                value,
            }),
        })
        .map(Record::Target)
        .message("while parsing mdt")
}

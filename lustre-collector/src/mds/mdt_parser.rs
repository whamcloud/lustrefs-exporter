// Copyright (c) 2024 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    ExportStats,
    base_parsers::{digits, param, param_period, period, target},
    exports_parser::exports_stats,
    oss::obdfilter_parser::{EXPORTS, EXPORTS_PARAMS},
    stats_parser::stats,
    types::{Param, Record, Stat, Target, TargetStat, TargetStats, TargetVariant},
};
use combine::{
    Parser, attempt, choice,
    error::ParseError,
    parser::char::{newline, string},
    stream::Stream,
};

pub(crate) const STATS: &str = "md_stats";
pub(crate) const NUM_EXPORTS: &str = "num_exports";

enum MdtStat {
    Stats(Vec<Stat>),
    NumExports(u64),
    ExportStats(Vec<ExportStats>),
}

fn mdt_stat<I>() -> impl Parser<I, Output = (Param, MdtStat)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    choice((
        (
            param(NUM_EXPORTS),
            digits().skip(newline()).map(MdtStat::NumExports),
        ),
        (param(STATS), stats().map(MdtStat::Stats)).message("while parsing mdt_stat"),
        (
            param_period(EXPORTS),
            exports_stats().map(MdtStat::ExportStats),
        ),
    ))
}

pub(crate) fn params() -> Vec<String> {
    [
        format!("mdt.*.{STATS}"),
        format!("mdt.*MDT*.{NUM_EXPORTS}"),
        format!("mdt.*MDT*.{EXPORTS_PARAMS}"),
    ]
    .into_iter()
    .collect()
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

pub(crate) fn parse<I>() -> impl Parser<I, Output = Record>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (target_name(), mdt_stat())
        .map(|(target, (param, value))| match value {
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

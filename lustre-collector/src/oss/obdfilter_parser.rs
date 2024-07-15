// Copyright (c) 2021 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    base_parsers::{digits, param, param_period, period, target},
    exports_parser::exports_stats,
    oss::job_stats,
    stats_parser::stats,
    types::{JobStatOst, Param, Record, Stat, Target, TargetStat, TargetStats, TargetVariant},
    ExportStats,
};
use combine::{
    choice,
    error::ParseError,
    parser::char::{newline, string},
    stream::Stream,
    Parser,
};

pub(crate) const JOBSTATS: &str = "job_stats";
pub(crate) const STATS: &str = "stats";

pub(crate) const NUM_EXPORTS: &str = "num_exports";
pub(crate) const TOT_DIRTY: &str = "tot_dirty";
pub(crate) const TOT_GRANTED: &str = "tot_granted";
pub(crate) const TOT_PENDING: &str = "tot_pending";

pub(crate) const EXPORTS: &str = "exports";
pub(crate) const EXPORTS_PARAMS: &str = "exports.*.stats";

pub(crate) const OBD_STATS: [&str; 7] = [
    JOBSTATS,
    STATS,
    NUM_EXPORTS,
    TOT_DIRTY,
    TOT_GRANTED,
    TOT_PENDING,
    EXPORTS_PARAMS,
];

pub(crate) const OBD_STATS_NO_JOBSTATS: [&str; 6] = [
    STATS,
    NUM_EXPORTS,
    TOT_DIRTY,
    TOT_GRANTED,
    TOT_PENDING,
    EXPORTS_PARAMS,
];

pub(crate) const OBD_STATS_JOBSTATS_ONLY: [&str; 1] = [JOBSTATS];
/// Takes OBD_STATS and produces a list of params for
/// consumption in proper ltcl get_param format.
pub(crate) fn obd_params() -> Vec<String> {
    OBD_STATS
        .iter()
        .map(|x| format!("obdfilter.*OST*.{x}"))
        .collect()
}

/// Takes OBD_STATS and produces a list of params for
/// consumption in proper ltcl get_param format.
pub(crate) fn obd_params_no_jobstats() -> Vec<String> {
    OBD_STATS_NO_JOBSTATS
        .iter()
        .map(|x| format!("obdfilter.*OST*.{x}"))
        .collect()
}
pub(crate) fn obd_params_jobstats_only() -> Vec<String> {
    OBD_STATS_JOBSTATS_ONLY
        .iter()
        .map(|x| format!("obdfilter.*OST*.{x}"))
        .collect()
}

/// Parses the name of a target
fn target_name<I>() -> impl Parser<I, Output = Target>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (string("obdfilter").skip(period()), target().skip(period()))
        .map(|(_, x)| x)
        .message("while parsing target_name")
}

#[derive(Debug)]
enum ObdfilterStat {
    JobStats(Option<Vec<JobStatOst>>),
    Stats(Vec<Stat>),
    ExportStats(Vec<ExportStats>),
    NumExports(u64),
    TotDirty(u64),
    TotGranted(u64),
    TotPending(u64),
}

fn obdfilter_stat<I>() -> impl Parser<I, Output = (Param, ObdfilterStat)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    choice((
        (
            param(JOBSTATS),
            job_stats::parse().map(ObdfilterStat::JobStats),
        ),
        (param(STATS), stats().map(ObdfilterStat::Stats)),
        (
            param(NUM_EXPORTS),
            digits().skip(newline()).map(ObdfilterStat::NumExports),
        ),
        (
            param(TOT_DIRTY),
            digits().skip(newline()).map(ObdfilterStat::TotDirty),
        ),
        (
            param(TOT_GRANTED),
            digits().skip(newline()).map(ObdfilterStat::TotGranted),
        ),
        (
            param(TOT_PENDING),
            digits().skip(newline()).map(ObdfilterStat::TotPending),
        ),
        (
            param_period(EXPORTS),
            exports_stats().map(ObdfilterStat::ExportStats),
        ),
    ))
    .message("while parsing obdfilter")
}

pub(crate) fn parse<I>() -> impl Parser<I, Output = Record>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (target_name(), obdfilter_stat())
        .map(|(target, (param, value))| match value {
            ObdfilterStat::JobStats(value) => TargetStats::JobStatsOst(TargetStat {
                kind: TargetVariant::Ost,
                target,
                param,
                value,
            }),
            ObdfilterStat::Stats(value) => TargetStats::Stats(TargetStat {
                kind: TargetVariant::Ost,
                target,
                param,
                value,
            }),
            ObdfilterStat::NumExports(value) => TargetStats::NumExports(TargetStat {
                kind: TargetVariant::Ost,
                target,
                param,
                value,
            }),
            ObdfilterStat::TotDirty(value) => TargetStats::TotDirty(TargetStat {
                kind: TargetVariant::Ost,
                target,
                param,
                value,
            }),
            ObdfilterStat::TotGranted(value) => TargetStats::TotGranted(TargetStat {
                kind: TargetVariant::Ost,
                target,
                param,
                value,
            }),
            ObdfilterStat::TotPending(value) => TargetStats::TotPending(TargetStat {
                kind: TargetVariant::Ost,
                target,
                param,
                value,
            }),
            ObdfilterStat::ExportStats(value) => TargetStats::ExportStats(TargetStat {
                kind: TargetVariant::Ost,
                target,
                param,
                value,
            }),
        })
        .map(Record::Target)
        .message("while parsing obdfilter")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_target_name() {
        let result = target_name().parse("obdfilter.fs-OST0000.num_exports=");

        assert_eq!(
            result,
            Ok((Target("fs-OST0000".to_string()), "num_exports="))
        );
    }
}

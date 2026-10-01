// Copyright (c) 2024 DDN. All rights reserved.
// Copyright (c) 2026 Google LLC
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

//! Client (llite) parameters: `llite.<fsname-instance>.<param>`.

mod extents_stats_parser;
mod read_ahead_stats_parser;

use crate::{
    Param, Record, Stat, Target, TargetStats,
    base_parsers::{param, period, target, till_newline},
    key_value_parser::key_values,
    stats_parser::stats,
    time::{StatsHeader, time_triple},
    types::{ExtentsBucket, KeyValue, LliteTargetStat, ProcessExtents, RwStats},
};
use combine::{
    ParseError, Parser, Stream, attempt, choice,
    parser::char::{newline, string},
};

pub(crate) const LLITE: &str = "llite";
pub(crate) const STATS: &str = "stats";
pub(crate) const READ_AHEAD_STATS: &str = "read_ahead_stats";
pub(crate) const EXTENTS_STATS: &str = "extents_stats";
pub(crate) const EXTENTS_STATS_PER_PROCESS: &str = "extents_stats_per_process";
pub(crate) const STATAHEAD_STATS: &str = "statahead_stats";
pub(crate) const UNSTABLE_STATS: &str = "unstable_stats";

pub(crate) fn params() -> Vec<String> {
    [
        STATS,
        READ_AHEAD_STATS,
        EXTENTS_STATS,
        EXTENTS_STATS_PER_PROCESS,
        STATAHEAD_STATS,
        UNSTABLE_STATS,
    ]
    .into_iter()
    .map(|x| format!("{LLITE}.*.{x}"))
    .collect()
}

/// Prints `disabled` plus one hint line unless `ll_rw_stats_on` is set.
pub(crate) fn rw_stats<I, P>(table: P) -> impl Parser<I, Output = RwStats<P::Output>>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
    P: Parser<I>,
{
    newline().with(choice((
        attempt(string("disabled"))
            .skip(newline())
            .skip(till_newline())
            .skip(newline())
            .map(|_| RwStats::Disabled),
        (time_triple(), table).map(|(header, value)| RwStats::Enabled { header, value }),
    )))
}

fn target_name<I>() -> impl Parser<I, Output = Target>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (string(LLITE).skip(period()), target().skip(period()))
        .map(|(_, x)| x)
        .message("while parsing llite target_name")
}

enum LliteStat {
    Stats(StatsHeader, Vec<Stat>),
    ReadAheadStats(StatsHeader, Vec<Stat>),
    ExtentsStats(RwStats<Vec<ExtentsBucket>>),
    ExtentsStatsPerProcess(RwStats<Vec<ProcessExtents>>),
    StataheadStats(Vec<KeyValue>),
    UnstableStats(Vec<KeyValue>),
}

fn llite_stat<I>() -> impl Parser<I, Output = (Param, LliteStat)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    choice((
        (param(STATS), stats().map(|(h, v)| LliteStat::Stats(h, v))),
        (
            param(READ_AHEAD_STATS),
            read_ahead_stats_parser::read_ahead_stats()
                .map(|(h, v)| LliteStat::ReadAheadStats(h, v)),
        ),
        (
            param(EXTENTS_STATS_PER_PROCESS),
            extents_stats_parser::extents_stats_per_process()
                .map(LliteStat::ExtentsStatsPerProcess),
        ),
        (
            param(EXTENTS_STATS),
            extents_stats_parser::extents_stats().map(LliteStat::ExtentsStats),
        ),
        (
            param(STATAHEAD_STATS),
            key_values().map(LliteStat::StataheadStats),
        ),
        (
            param(UNSTABLE_STATS),
            key_values().map(LliteStat::UnstableStats),
        ),
    ))
    .message("while parsing llite_stat")
}

pub(crate) fn parse<I>() -> impl Parser<I, Output = Record>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (target_name(), llite_stat())
        .map(|(target, (param, value))| match value {
            LliteStat::Stats(header, stats) => TargetStats::Llite(crate::types::LliteStat {
                target,
                param,
                stats,
                header,
            }),
            LliteStat::ReadAheadStats(header, stats) => {
                TargetStats::LliteReadAheadStats(crate::types::LliteStat {
                    target,
                    param,
                    stats,
                    header,
                })
            }
            LliteStat::ExtentsStats(value) => TargetStats::LliteExtentsStats(LliteTargetStat {
                target,
                param,
                value,
            }),
            LliteStat::ExtentsStatsPerProcess(value) => {
                TargetStats::LliteExtentsStatsPerProcess(LliteTargetStat {
                    target,
                    param,
                    value,
                })
            }
            LliteStat::StataheadStats(value) => TargetStats::LliteStataheadStats(LliteTargetStat {
                target,
                param,
                value,
            }),
            LliteStat::UnstableStats(value) => TargetStats::LliteUnstableStats(LliteTargetStat {
                target,
                param,
                value,
            }),
        })
        .map(Record::Target)
        .message("while parsing llite")
}

#[cfg(test)]
mod tests {
    use super::*;
    use combine::many;
    use insta::assert_debug_snapshot;

    #[test]
    fn test_parse() {
        let x = r#"llite.ai400x2-ffff9440f1003000.stats=
snapshot_time             1689697369.331040915 secs.nsecs
ioctl                     2 samples [reqs]
open                      13812423 samples [usec] 1 725287 1027077752 8835364169944
close                     13812423 samples [usec] 47 778498 1320315612 17542973849370
readdir                   12 samples [usec] 0 4647 6715 22456295
getattr                   14812440 samples [usec] 2 320411 1317584841 2110166912709
unlink                    6906208 samples [usec] 117 749323 1386719680 23443327087798
mkdir                     7906554 samples [usec] 104 1529199 20996782592 1837945636486522
rmdir                     6939862 samples [usec] 95 646028 16617944601 635123583760591
mknod                     6906208 samples [usec] 119 775827 1454511094 10119157242014
statfs                    7 samples [usec] 147 197 1236 220284
inode_permission          251887103 samples [usec] 0 14235 178199279 1102415701
opencount                 13812424 samples [reqs] 1 2 20718632 34531048
openclosetime             6906208 samples [usec] 2225920 34405427 163169641155255 11416538743473681487
"#;

        let result: (Vec<_>, _) = many(parse()).parse(x).unwrap();

        assert_debug_snapshot!(result)
    }
}

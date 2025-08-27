// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    base_parsers::{param, period, target},
    mds::mdt_parser::STATS as MD_STATS,
    stats_parser::stats,
    types::{Param, Record, Stat, Target, TargetStat, TargetStats, TargetVariant},
};
use combine::{Parser, choice, error::ParseError, parser::char::string, stream::Stream};

const DT_STATS: &str = "dt_stats";
pub(crate) const NODEMAP: &str = "nodemap";

#[derive(Debug)]
enum NodemapStat {
    Md(Vec<Stat>),
    Dt(Vec<Stat>),
}

pub(crate) fn params() -> Vec<String> {
    vec![
        format!("{NODEMAP}.*.{DT_STATS}"),
        format!("{NODEMAP}.*.{MD_STATS}"),
    ]
}

fn nodemap_stat<I>() -> impl Parser<I, Output = (Param, NodemapStat)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    choice((
        (param(MD_STATS), stats().map(NodemapStat::Md)),
        (param(DT_STATS), stats().map(NodemapStat::Dt)),
    ))
    .message("while parsing nodemap params")
}

/// Parses the name of a target
fn target_name<I>() -> impl Parser<I, Output = Target>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (string("nodemap").skip(period()), target().skip(period()))
        .map(|(_, x)| x)
        .message("while parsing target_name")
}

pub(crate) fn parse<I>() -> impl Parser<I, Output = Record>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (target_name(), nodemap_stat())
        .map(|(target, (param, value))| match value {
            NodemapStat::Dt(value) => TargetStats::Stats(TargetStat {
                kind: TargetVariant::Ost,
                target,
                param,
                value,
            }),
            NodemapStat::Md(value) => TargetStats::Stats(TargetStat {
                kind: TargetVariant::Mdt,
                target,
                param,
                value,
            }),
        })
        .map(Record::Target)
        .message("while parsing nodemap")
}

#[cfg(test)]
mod tests {
    use crate::parser::parse;
    use combine::EasyParser as _;

    #[test]
    // Test output included in LU-18950 patch
    fn lu18950() {
        let xs = r#"nodemap.c0.md_stats=
snapshot_time             1746601510.804589748 secs.nsecs
start_time                1746601507.675857245 secs.nsecs
elapsed_time              3.128732503 secs.nsecs
open                      2 samples [usecs] 14 134 148 18152
close                     2 samples [usecs] 9 19 28 442
mknod                     1 samples [usecs] 105 105 105 11025
unlink                    1 samples [usecs] 95 95 95 9025
getattr                   2 samples [usecs] 3 6 9 45
sync                      1 samples [usecs] 124 124 124 15376
nodemap.c0.dt_stats=
snapshot_time             1746601510.814726699 secs.nsecs
start_time                1746601507.675852028 secs.nsecs
elapsed_time              3.138874671 secs.nsecs
write_bytes               1 samples [bytes] 1024 1024 1024 1048576
write                     1 samples [usecs] 18 18 18 324
sync                      1 samples [usecs] 88 88 88 7744
"#;

        let (records, state) = parse().easy_parse(xs).unwrap();

        insta::assert_snapshot!(state, @"");
        insta::assert_debug_snapshot!(records, @r#"
        [
            Target(
                Stats(
                    TargetStat {
                        kind: Mdt,
                        param: Param(
                            "md_stats",
                        ),
                        target: Target(
                            "c0",
                        ),
                        value: [
                            Stat {
                                name: "open",
                                units: "usecs",
                                samples: 2,
                                min: Some(
                                    14,
                                ),
                                max: Some(
                                    134,
                                ),
                                sum: Some(
                                    148,
                                ),
                                sumsquare: Some(
                                    18152,
                                ),
                            },
                            Stat {
                                name: "close",
                                units: "usecs",
                                samples: 2,
                                min: Some(
                                    9,
                                ),
                                max: Some(
                                    19,
                                ),
                                sum: Some(
                                    28,
                                ),
                                sumsquare: Some(
                                    442,
                                ),
                            },
                            Stat {
                                name: "mknod",
                                units: "usecs",
                                samples: 1,
                                min: Some(
                                    105,
                                ),
                                max: Some(
                                    105,
                                ),
                                sum: Some(
                                    105,
                                ),
                                sumsquare: Some(
                                    11025,
                                ),
                            },
                            Stat {
                                name: "unlink",
                                units: "usecs",
                                samples: 1,
                                min: Some(
                                    95,
                                ),
                                max: Some(
                                    95,
                                ),
                                sum: Some(
                                    95,
                                ),
                                sumsquare: Some(
                                    9025,
                                ),
                            },
                            Stat {
                                name: "getattr",
                                units: "usecs",
                                samples: 2,
                                min: Some(
                                    3,
                                ),
                                max: Some(
                                    6,
                                ),
                                sum: Some(
                                    9,
                                ),
                                sumsquare: Some(
                                    45,
                                ),
                            },
                            Stat {
                                name: "sync",
                                units: "usecs",
                                samples: 1,
                                min: Some(
                                    124,
                                ),
                                max: Some(
                                    124,
                                ),
                                sum: Some(
                                    124,
                                ),
                                sumsquare: Some(
                                    15376,
                                ),
                            },
                        ],
                    },
                ),
            ),
            Target(
                Stats(
                    TargetStat {
                        kind: Ost,
                        param: Param(
                            "dt_stats",
                        ),
                        target: Target(
                            "c0",
                        ),
                        value: [
                            Stat {
                                name: "write_bytes",
                                units: "bytes",
                                samples: 1,
                                min: Some(
                                    1024,
                                ),
                                max: Some(
                                    1024,
                                ),
                                sum: Some(
                                    1024,
                                ),
                                sumsquare: Some(
                                    1048576,
                                ),
                            },
                            Stat {
                                name: "write",
                                units: "usecs",
                                samples: 1,
                                min: Some(
                                    18,
                                ),
                                max: Some(
                                    18,
                                ),
                                sum: Some(
                                    18,
                                ),
                                sumsquare: Some(
                                    324,
                                ),
                            },
                            Stat {
                                name: "sync",
                                units: "usecs",
                                samples: 1,
                                min: Some(
                                    88,
                                ),
                                max: Some(
                                    88,
                                ),
                                sum: Some(
                                    88,
                                ),
                                sumsquare: Some(
                                    7744,
                                ),
                            },
                        ],
                    },
                ),
            ),
        ]
        "#);
    }
}

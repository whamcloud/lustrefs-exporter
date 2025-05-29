// Copyright (c) 2021 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    base_parsers::{digits, param, period, target},
    stats_parser::stats,
    types::{Param, Record, Stat, Target, TargetStat, TargetStats, TargetVariant},
};
use combine::{
    Parser, attempt, choice,
    error::ParseError,
    parser::char::{newline, string},
    stream::Stream,
};

pub const STATS: &str = "stats";
pub const THREADS_MIN: &str = "threads_min";
pub const THREADS_MAX: &str = "threads_max";
pub const THREADS_STARTED: &str = "threads_started";
pub const NUM_EXPORTS: &str = "num_exports";

pub fn params() -> Vec<String> {
    [
        format!("mgs.*.mgs.{STATS}"),
        format!("mgs.*.mgs.{THREADS_MAX}"),
        format!("mgs.*.mgs.{THREADS_MIN}"),
        format!("mgs.*.mgs.{THREADS_STARTED}"),
        format!("mgs.*.{NUM_EXPORTS}"),
    ]
    .iter()
    .map(|x| x.to_owned())
    .collect::<Vec<_>>()
}

#[derive(Debug)]
enum MgsStat {
    Stats(Vec<Stat>),
    ThreadsMin(u64),
    ThreadsMax(u64),
    ThreadsStarted(u64),
    NumExports(u64),
}

/// Parses the name of a target
fn target_name<I>() -> impl Parser<I, Output = Target>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        attempt(string("mgs")).skip(period()),
        target().skip(period()),
    )
        .map(|(_, x)| x)
        .message("while parsing target_name")
}

fn mgs_stat<I>() -> impl Parser<I, Output = (Param, MgsStat)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    choice((
        (
            param(NUM_EXPORTS),
            digits().skip(newline()).map(MgsStat::NumExports),
        ),
        (
            string("mgs").skip(period()),
            choice((
                (param(STATS), stats().map(MgsStat::Stats)),
                (
                    param(THREADS_MIN),
                    digits().skip(newline()).map(MgsStat::ThreadsMin),
                ),
                (
                    param(THREADS_MAX),
                    digits().skip(newline()).map(MgsStat::ThreadsMax),
                ),
                (
                    param(THREADS_STARTED),
                    digits().skip(newline()).map(MgsStat::ThreadsStarted),
                ),
            )),
        )
            .map(|(_, (y, z))| (y, z)),
    ))
    .message("while parsing mgs stats")
}

pub fn parse<I>() -> impl Parser<I, Output = Record>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (target_name(), mgs_stat())
        .map(|(target, (param, value))| match value {
            MgsStat::Stats(value) => TargetStats::Stats(TargetStat {
                kind: TargetVariant::Mgt,
                target,
                param,
                value,
            }),
            MgsStat::NumExports(value) => TargetStats::NumExports(TargetStat {
                kind: TargetVariant::Mgt,
                target,
                param,
                value,
            }),
            MgsStat::ThreadsMin(value) => TargetStats::ThreadsMin(TargetStat {
                kind: TargetVariant::Mgt,
                target,
                param,
                value,
            }),
            MgsStat::ThreadsMax(value) => TargetStats::ThreadsMax(TargetStat {
                kind: TargetVariant::Mgt,
                target,
                param,
                value,
            }),
            MgsStat::ThreadsStarted(value) => TargetStats::ThreadsStarted(TargetStat {
                kind: TargetVariant::Mgt,
                target,
                param,
                value,
            }),
        })
        .map(Record::Target)
        .message("while parsing mgs params")
}

#[cfg(test)]
mod tests {
    use super::*;
    use combine::{many, parser::EasyParser};
    use insta::assert_debug_snapshot;

    #[test]
    fn test_parse() {
        let x = r#"mgs.MGS.mgs.stats=
snapshot_time             1596728874.484750908 secs.nsecs
req_waittime              31280 samples [usec] 11 2695 5020274 1032267156
req_qdepth                31280 samples [reqs] 0 1 56 56
req_active                31280 samples [reqs] 1 2 36625 47315
req_timeout               31280 samples [sec] 1 10 31289 31379
reqbuf_avail              85192 samples [bufs] 62 64 5364658 337866142
ldlm_plain_enqueue        201 samples [reqs] 1 1 201 201
mgs_connect               9 samples [usec] 52 5165 19362 66639088
mgs_disconnect            4 samples [usec] 50 92 265 18709
mgs_target_reg            90 samples [usec] 874 163383 1262544 91852108168
mgs_config_read           41 samples [usec] 41 2203 26823 32448779
obd_ping                  30339 samples [usec] 3 4398 1552005 134387261
llog_origin_handle_open   153 samples [usec] 29 16443 25516 270992222
llog_origin_handle_next_block 298 samples [usec] 24 31952 141030 2788155300
llog_origin_handle_read_header 145 samples [usec] 25 44125 192095 4905765639
mgs.MGS.mgs.threads_max=32
mgs.MGS.mgs.threads_min=3
mgs.MGS.mgs.threads_started=4
mgs.MGS.num_exports=5
"#;

        let result: (Vec<_>, _) = many(parse()).easy_parse(x).unwrap();

        assert_debug_snapshot!(result)
    }
}

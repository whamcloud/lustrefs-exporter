// Copyright (c) 2024 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    base_parsers::{equals, period},
    stats_parser::stats,
    types::{Param, Record, Stat, TargetStats},
    OssStat,
};
use combine::{attempt, choice, error::ParseError, parser::char::string, stream::Stream, Parser};

const OSS: &str = "OSS";
const STATS: &str = "stats";
pub(crate) const OST: &str = "ost";

pub(crate) const OST_IO: &str = "ost_io";
pub(crate) const OST_CREATE: &str = "ost_create";
pub(crate) const OST_OUT: &str = "ost_out";
pub(crate) const OST_SEQ: &str = "ost_seq";

pub(crate) const OST_STATS: [&str; 5] = [OST, OST_IO, OST_CREATE, OST_OUT, OST_SEQ];

/// Takes [`OST_STATS`] and produces a list of params for
/// consumption in proper ltcl get_param format.
pub(crate) fn params() -> Vec<String> {
    OST_STATS
        .iter()
        .map(|x| format!("{OST}.{OSS}.{x}.{STATS}"))
        .collect()
}

fn oss_prefix<I>() -> impl Parser<I, Output = ()>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (string(OST).skip(period()))
        .with(string(OSS).skip(period()))
        .map(|_| ())
        .message("while parsing `oss_prefix`")
}

fn param_non_final<I>(x: &'static str) -> impl Parser<I, Output = Param>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    attempt(string(x).skip(period()))
        .skip(string(STATS).skip(equals()))
        .map(|x| Param(x.to_string()))
        .message("while parsing `oss_suffix`")
}

fn oss_stat<I>() -> impl Parser<I, Output = (Param, Vec<Stat>)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        choice((
            param_non_final(OST),
            param_non_final(OST_IO),
            param_non_final(OST_CREATE),
            param_non_final(OST_OUT),
            param_non_final(OST_SEQ),
        )),
        stats(),
    )
        .message("while parsing `oss_stat`")
}

pub(crate) fn parse<'a, I>() -> impl Parser<I, Output = Record<'a>>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    oss_prefix()
        .with(oss_stat())
        .map(|(param, stats)| TargetStats::Oss(OssStat { param, stats }))
        .map(Record::Target)
        .message("while parsing oss")
}

#[cfg(test)]
mod tests {
    use super::*;
    use combine::{many, parser::EasyParser};
    use insta::assert_debug_snapshot;

    #[test]
    fn test_parse() {
        let x = r#"ost.OSS.ost.stats=
snapshot_time             1688128253.497763049 secs.nsecs
req_waittime              18419628 samples [usec] 2 40983 305482965 25043535105
req_qdepth                18419628 samples [reqs] 0 34 99937 130635
req_active                18419628 samples [reqs] 1 36 69585063 634492353
req_timeout               18419628 samples [sec] 1 15 276294334 4144414654
reqbuf_avail              38185151 samples [bufs] 60 64 2438170078 155685175822
ldlm_glimpse_enqueue      9257180 samples [reqs] 1 1 9257180 9257180
ldlm_extent_enqueue       19856 samples [reqs] 1 1 19856 19856
ost_create                144904 samples [usec] 6 16594 98795730 85661707326
ost_destroy               8988941 samples [usec] 89 173579 5160119682 8184502010174
ost_get_info              8 samples [usec] 540 3603 10971 28145019
ost_connect               341 samples [usec] 21 903 24182 2818080
ost_disconnect            331 samples [usec] 23 524 39358 7068516
ost_sync                  4510 samples [usec] 3 10945 997271 2117171965
ost_set_info              28 samples [usec] 9 34 606 14594
obd_ping                  3529 samples [usec] 3 12431 60722 155336592
ost.OSS.ost_io.stats=
snapshot_time             1688128269.170769339 secs.nsecs
req_waittime              3398592545 samples [usec] 2 585517 95316362073 32500246129015
req_qdepth                3398592545 samples [reqs] 0 53 90676247 112259319
req_active                3398592545 samples [reqs] 1 82 55496806665 1427593461517
req_timeout               3398592545 samples [sec] 15 15 50978888175 764683322625
reqbuf_avail              7234158916 samples [bufs] 55 64 461878663298 29490702443182
ost_read                  2447557926 samples [usec] 23 138321 1871024223288 4497819893384848
ost_write                 951033049 samples [usec] 59 1247713 2749050524782 100048363896296658
ost_punch                 1515 samples [usec] 16 4883 63967 29511205
"#;

        let result: (Vec<_>, _) = many(parse()).easy_parse(x).unwrap();

        assert_debug_snapshot!(result)
    }
}

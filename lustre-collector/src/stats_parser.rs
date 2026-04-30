// Copyright (c) 2021 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    base_parsers::{digits, not_words, word},
    ldlm::LDLM,
    llite::LLITE,
    mdd_parser::MDD,
    mds::mds_parser::MDS,
    nodemap::NODEMAP,
    osc_parser::OSC,
    osd_parser::OSD,
    oss::oss_parser::OST,
    quota::QMT,
    time::{StatsHeader, time_triple},
    types::Stat,
};
use combine::{
    Parser, between,
    error::ParseError,
    many, optional,
    parser::{
        char::{newline, spaces, string},
        choice::or,
    },
    stream::Stream,
    token,
};

fn name_count_units<I>() -> impl Parser<I, Output = (String, u64, String)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        not_words(&[
            "obdfilter",
            "mgs",
            "mdt",
            LDLM,
            OST,
            LLITE,
            MDS,
            MDD,
            NODEMAP,
            QMT,
            OSC,
            OSD,
        ])
        .skip(spaces()),
        digits(),
        spaces().with(string("samples")),
        spaces().with(between(token('['), token(']'), word())),
    )
        .map(|(x, y, _, z)| (x, y, z))
}

fn min_max_sum<I>() -> impl Parser<I, Output = (u64, u64, u64)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        spaces().with(digits()),
        spaces().with(digits()),
        spaces().with(digits()),
    )
}

fn sum_sq<I>() -> impl Parser<I, Output = u64>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    spaces().with(digits())
}

pub(crate) fn stat<I>() -> impl Parser<I, Output = Stat>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        name_count_units(),
        or(
            newline().map(|_| (None, None)),
            (
                min_max_sum().map(Some),
                or(newline().map(|_| None), sum_sq().map(Some).skip(newline())),
            ),
        ),
    )
        .map(
            |((name, samples, units), (min_max, sum))| match (min_max, sum) {
                (Some((min, max, sum)), Some(sumsquare)) => Stat {
                    name,
                    samples,
                    units,
                    min: Some(min),
                    max: Some(max),
                    sum: Some(sum),
                    sumsquare: Some(sumsquare),
                },
                (Some((min, max, sum)), None) => Stat {
                    name,
                    samples,
                    units,
                    min: Some(min),
                    max: Some(max),
                    sum: Some(sum),
                    sumsquare: None,
                },
                (None, _) => Stat {
                    name,
                    samples,
                    units,
                    min: None,
                    max: None,
                    sum: None,
                    sumsquare: None,
                },
            },
        )
}

pub(crate) fn stats<I>() -> impl Parser<I, Output = (StatsHeader, Vec<Stat>)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (optional(newline()).with(time_triple()), many(stat())).map(|(header, xs)| (header, xs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::{assert_compact_debug_snapshot, assert_debug_snapshot};

    #[test]
    fn test_name_count_units() {
        let x = r#"create                    726 samples [reqs]
"#;

        let result = name_count_units().parse(x);

        assert_compact_debug_snapshot!(
            result,
            @r#"Ok((("create", 726, "reqs"), "\n"))"#
        );
    }

    #[test]
    fn test_stat_no_sumsquare() {
        let x = r#"cache_miss                21108 samples [pages] 1 1 21108
"#;

        let result = stat().parse(x);

        assert_compact_debug_snapshot!(
            result, @r#"Ok((Stat { name: "cache_miss", units: "pages", samples: 21108, min: Some(1), max: Some(1), sum: Some(21108), sumsquare: None }, ""))"#
        );
    }

    #[test]
    fn test_stat_cache_hit() {
        let x = r#"cache_hit                99 samples [pages] 1 1 99 10
"#;

        let result = stat().parse(x);

        assert_compact_debug_snapshot!(
            result, @r#"Ok((Stat { name: "cache_hit", units: "pages", samples: 99, min: Some(1), max: Some(1), sum: Some(99), sumsquare: Some(10) }, ""))"#
        );
    }

    #[test]
    fn test_stat_cache_hit_all_none() {
        let x = r#"cache_hit                123 samples [pages]
"#;

        let result = stat().parse(x);

        assert_compact_debug_snapshot!(
            result, @r#"Ok((Stat { name: "cache_hit", units: "pages", samples: 123, min: None, max: None, sum: None, sumsquare: None }, ""))"#
        );
    }

    #[test]
    fn test_stat() {
        let x = r#"obd_ping                  1108 samples [usec] 15 72 47014 2156132
"#;

        let result = stat().parse(x);

        assert_compact_debug_snapshot!(
            result,
            @r#"Ok((Stat { name: "obd_ping", units: "usec", samples: 1108, min: Some(15), max: Some(72), sum: Some(47014), sumsquare: Some(2156132) }, ""))"#

        );
    }

    #[test]
    fn test_stats() {
        let x = r#"
snapshot_time             1534770326.579119384 secs.nsecs
write_bytes               9 samples [bytes] 98303 4194304 33554431
create                    4 samples [reqs]
statfs                    5634 samples [reqs]
get_info                  2 samples [reqs]
connect                   4 samples [reqs]
reconnect                 1 samples [reqs]
disconnect                3 samples [reqs]
statfs                    18 samples [reqs]
preprw                    9 samples [reqs]
commitrw                  9 samples [reqs]
ping                      1075 samples [reqs]
get_page                  13 samples [usecs] 0 3 6 18
cache_access              4 samples [pages] 1 25 52
cache_hit                 4 samples [pages] 1 25 52
many_credits              1 samples [reqs] 1 1 1
"#;

        let result = stats().parse(x).unwrap();

        assert_debug_snapshot!(result);
    }

    #[test]
    fn test_mdstats() {
        let x = r#"
snapshot_time             1566007540.707634939 secs.nsecs
statfs                    16360 samples [reqs]
"#;

        let result = stats().parse(x).unwrap();

        assert_debug_snapshot!(result);
    }

    #[test]
    fn test_empty_mdstats() {
        let x = r#"
snapshot_time             1581546409.693472737 secs.nsecs
"#;

        let result = stats().parse(x).unwrap();

        assert_debug_snapshot!(result);
    }

    #[test]
    fn test_mdstats_with_latency() {
        let x = r#"
snapshot_time             1784534117.627631272 secs.nsecs
start_time                1782813468.925101302 secs.nsecs
elapsed_time              1720648.702529970 secs.nsecs
open                      24 samples [usecs] 20 11267 13093 127368057
close                     24 samples [usecs] 13 67 745 27517
mknod                     3 samples [usecs] 557 11242 18610 173082534
mkdir                     5 samples [usecs] 94 22555 23009 508780805
getattr                   166 samples [usecs] 3 81 1822 51452
setattr                   6 samples [usecs] 31 52256 81884 3174388656
statfs                    2352443 samples [usecs] 0 2266 18990357 206437167
"#;

        let result = stats().parse(x).unwrap();

        insta::assert_debug_snapshot!(result, @r#"
        (
            (
                StatsHeader {
                    snapshot_time: "1784534117.627631272",
                    start_time: Some(
                        "1782813468.925101302",
                    ),
                },
                [
                    Stat {
                        name: "open",
                        units: "usecs",
                        samples: 24,
                        min: Some(
                            20,
                        ),
                        max: Some(
                            11267,
                        ),
                        sum: Some(
                            13093,
                        ),
                        sumsquare: Some(
                            127368057,
                        ),
                    },
                    Stat {
                        name: "close",
                        units: "usecs",
                        samples: 24,
                        min: Some(
                            13,
                        ),
                        max: Some(
                            67,
                        ),
                        sum: Some(
                            745,
                        ),
                        sumsquare: Some(
                            27517,
                        ),
                    },
                    Stat {
                        name: "mknod",
                        units: "usecs",
                        samples: 3,
                        min: Some(
                            557,
                        ),
                        max: Some(
                            11242,
                        ),
                        sum: Some(
                            18610,
                        ),
                        sumsquare: Some(
                            173082534,
                        ),
                    },
                    Stat {
                        name: "mkdir",
                        units: "usecs",
                        samples: 5,
                        min: Some(
                            94,
                        ),
                        max: Some(
                            22555,
                        ),
                        sum: Some(
                            23009,
                        ),
                        sumsquare: Some(
                            508780805,
                        ),
                    },
                    Stat {
                        name: "getattr",
                        units: "usecs",
                        samples: 166,
                        min: Some(
                            3,
                        ),
                        max: Some(
                            81,
                        ),
                        sum: Some(
                            1822,
                        ),
                        sumsquare: Some(
                            51452,
                        ),
                    },
                    Stat {
                        name: "setattr",
                        units: "usecs",
                        samples: 6,
                        min: Some(
                            31,
                        ),
                        max: Some(
                            52256,
                        ),
                        sum: Some(
                            81884,
                        ),
                        sumsquare: Some(
                            3174388656,
                        ),
                    },
                    Stat {
                        name: "statfs",
                        units: "usecs",
                        samples: 2352443,
                        min: Some(
                            0,
                        ),
                        max: Some(
                            2266,
                        ),
                        sum: Some(
                            18990357,
                        ),
                        sumsquare: Some(
                            206437167,
                        ),
                    },
                ],
            ),
            "",
        )
        "#);
    }

    #[test]
    fn test_mdstats_after_reset() {
        let x = r#"
snapshot_time             1784534177.628150302 secs.nsecs
start_time                1784534117.627631272 secs.nsecs
elapsed_time              60.000519030 secs.nsecs
open                      2 samples [usecs] 25 89 114 8881
close                     2 samples [usecs] 15 18 33 549
getattr                   5 samples [usecs] 4 12 35 293
statfs                    1247 samples [usecs] 2 45 8912 79843
"#;

        let result = stats().parse(x).unwrap();

        insta::assert_debug_snapshot!(result, @r#"
        (
            (
                StatsHeader {
                    snapshot_time: "1784534177.628150302",
                    start_time: Some(
                        "1784534117.627631272",
                    ),
                },
                [
                    Stat {
                        name: "open",
                        units: "usecs",
                        samples: 2,
                        min: Some(
                            25,
                        ),
                        max: Some(
                            89,
                        ),
                        sum: Some(
                            114,
                        ),
                        sumsquare: Some(
                            8881,
                        ),
                    },
                    Stat {
                        name: "close",
                        units: "usecs",
                        samples: 2,
                        min: Some(
                            15,
                        ),
                        max: Some(
                            18,
                        ),
                        sum: Some(
                            33,
                        ),
                        sumsquare: Some(
                            549,
                        ),
                    },
                    Stat {
                        name: "getattr",
                        units: "usecs",
                        samples: 5,
                        min: Some(
                            4,
                        ),
                        max: Some(
                            12,
                        ),
                        sum: Some(
                            35,
                        ),
                        sumsquare: Some(
                            293,
                        ),
                    },
                    Stat {
                        name: "statfs",
                        units: "usecs",
                        samples: 1247,
                        min: Some(
                            2,
                        ),
                        max: Some(
                            45,
                        ),
                        sum: Some(
                            8912,
                        ),
                        sumsquare: Some(
                            79843,
                        ),
                    },
                ],
            ),
            "",
        )
        "#);
    }
}

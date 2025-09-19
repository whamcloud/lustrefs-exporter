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
    osd_parser::OSD,
    oss::oss_parser::OST,
    quota::QMT,
    time::time_triple,
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

pub(crate) fn stats<I>() -> impl Parser<I, Output = Vec<Stat>>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (optional(newline()).with(time_triple()), many(stat())).map(|(_, xs)| xs)
}

#[cfg(test)]
mod tests {
    use super::*;
    use insta::assert_debug_snapshot;

    #[test]
    fn test_name_count_units() {
        let x = r#"create                    726 samples [reqs]
"#;

        let result = name_count_units().parse(x);

        assert_debug_snapshot!(
            result,
            @r#"
        Ok(
            (
                (
                    "create",
                    726,
                    "reqs",
                ),
                "\n",
            ),
        )
        "#
        );
    }

    #[test]
    fn test_stat_no_sumsquare() {
        let x = r#"cache_miss                21108 samples [pages] 1 1 21108
"#;

        let result = stat().parse(x);

        assert_debug_snapshot!(
            result, @r#"
        Ok(
            (
                Stat {
                    name: "cache_miss",
                    units: "pages",
                    samples: 21108,
                    min: Some(
                        1,
                    ),
                    max: Some(
                        1,
                    ),
                    sum: Some(
                        21108,
                    ),
                    sumsquare: None,
                },
                "",
            ),
        )
        "#
        );
    }

    #[test]
    fn test_stat_cache_hit() {
        let x = r#"cache_hit                99 samples [pages] 1 1 99 10
"#;

        let result = stat().parse(x);

        assert_debug_snapshot!(
            result, @r#"
        Ok(
            (
                Stat {
                    name: "cache_hit",
                    units: "pages",
                    samples: 99,
                    min: Some(
                        1,
                    ),
                    max: Some(
                        1,
                    ),
                    sum: Some(
                        99,
                    ),
                    sumsquare: Some(
                        10,
                    ),
                },
                "",
            ),
        )
        "#
        );
    }

    #[test]
    fn test_stat_cache_hit_all_none() {
        let x = r#"cache_hit                123 samples [pages]
"#;

        let result = stat().parse(x);

        assert_debug_snapshot!(
            result, @r#"
        Ok(
            (
                Stat {
                    name: "cache_hit",
                    units: "pages",
                    samples: 123,
                    min: None,
                    max: None,
                    sum: None,
                    sumsquare: None,
                },
                "",
            ),
        )
        "#
        );
    }

    #[test]
    fn test_stat() {
        let x = r#"obd_ping                  1108 samples [usec] 15 72 47014 2156132
"#;

        let result = stat().parse(x);

        assert_debug_snapshot!(
            result,
            @r#"
        Ok(
            (
                Stat {
                    name: "obd_ping",
                    units: "usec",
                    samples: 1108,
                    min: Some(
                        15,
                    ),
                    max: Some(
                        72,
                    ),
                    sum: Some(
                        47014,
                    ),
                    sumsquare: Some(
                        2156132,
                    ),
                },
                "",
            ),
        )
        "#

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
}

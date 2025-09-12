// Copyright (c) 2024 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    QuotaKind, QuotaStat, QuotaStatLimits, QuotaStatOsd, QuotaStatUsage, QuotaStats,
    TargetQuotaStat,
    base_parsers::{digits, param, period, target},
    quota::QMT,
    types::{Param, Record, Target, TargetStats},
};
use combine::{
    Parser, Stream, between, choice, eof,
    error::ParseError,
    many1, optional,
    parser::{
        char::{newline, spaces, string},
        repeat::take_until,
    },
    token,
};

pub(crate) const USR_QUOTAS: &str = "usr";
pub(crate) const PRJ_QUOTAS: &str = "prj";
pub(crate) const GRP_QUOTAS: &str = "grp";
pub(crate) const QMT_STATS: [&str; 3] = [USR_QUOTAS, PRJ_QUOTAS, GRP_QUOTAS];

/// Takes QMT_STATS and produces a list of params for
/// consumption in proper ltcl get_param format.
pub(crate) fn params() -> Vec<String> {
    QMT_STATS
        .iter()
        .map(|x| format!("{QMT}.*.*.glb-{x}"))
        .collect()
}

/// Parses a target name
pub(crate) fn qmt_pool<I>() -> impl Parser<I, Output = (Target, Target)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        choice((
            string("md").skip(token('-')).map(|x| Target(x.to_string())),
            string("dt").skip(token('-')).map(|x| Target(x.to_string())),
        )),
        target(),
    )
}

/// Parses the name of a target
fn qmt_target<I>() -> impl Parser<I, Output = (Target, Target, Target)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (target().skip(period()), qmt_pool().skip(period()))
        .map(|(target, (manager, pool))| (target, manager, pool))
        .message("while parsing target_name")
}

/// Parses an ID line like "- id: 123\n"
fn id<I>() -> impl Parser<I, Output = u64>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    // `between(open, close, p)` is the `combine` equivalent of `winnow`'s `delimited(open, p, close)`
    between(
        (string("- id:"), spaces()), // open parser
        newline(),                   // close parser
        digits(),                    // main value parser
    )
}

/// Parses a limits block like "  limits: { hard: 1, soft: 2, granted: 3, time: 4 }"
fn limit<I>() -> impl Parser<I, Output = QuotaStatLimits>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    // Define parsers for each key-value field
    let hard = string("hard:").skip(spaces()).with(digits());
    let soft = string(", soft:").skip(spaces()).with(digits());
    let granted = string(", granted:").skip(spaces()).with(digits());
    let time = string(", time:").skip(spaces()).with(digits());

    // Combine the field parsers into a tuple and map the result to the struct
    let body = (hard, soft, granted, time);

    // Wrap the body parser with the prefix "  limits: { " and suffix " }"
    (spaces(), string("limits:"), spaces(), token('{'), spaces())
        .with(body)
        .map(
            |(hard, soft, granted, time): (u64, u64, u64, u64)| QuotaStatLimits {
                hard,
                soft,
                granted,
                time,
            },
        )
        // Discard prefix, keep body's result
        .skip((spaces(), token('}'))) // Keep body's result, discard suffix
}

/// Parses a limits block like "  limits: { hard: 1, soft: 2, granted: 3, time: 4 }"
fn usage<I>() -> impl Parser<I, Output = QuotaStatUsage>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    // Define parsers for each key-value field
    let inodes = string("inodes:").skip(spaces()).with(digits());
    let kbytes = string(", kbytes:").skip(spaces()).with(digits());

    // Combine the field parsers into a tuple and map the result to the struct
    let body = (inodes, kbytes);

    // Wrap the body parser with the prefix "  limits: { " and suffix " }"
    (spaces(), string("usage:"), spaces(), token('{'), spaces())
        .with(body)
        .map(|(inodes, kbytes): (u64, u64)| QuotaStatUsage { inodes, kbytes })
        // Discard prefix, keep body's result
        .skip((spaces(), token('}'))) // Keep body's result, discard suffix
}

pub(crate) fn quota_stats<I>() -> impl Parser<I, Output = Vec<QuotaStat>>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        optional(newline()), // If quota stats are present, the whole yaml blob will start on a newline
        take_until::<Vec<_>, _, _>(newline()), // But yaml header might not be indented, ignore it
        newline(),
        many1((id(), limit(), newline()).map(|(id, limits, _)| QuotaStat { id, limits })),
    )
        .skip(optional(newline()))
        .skip(optional(eof()))
        .map(|(_, _, _, qs)| qs)
}

pub(crate) fn quota_stats_osd<I>() -> impl Parser<I, Output = Vec<QuotaStatOsd>>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        optional(newline()), // If quota stats are present, the whole yaml blob will start on a newline
        take_until::<Vec<_>, _, _>(newline()), // But yaml header might not be indented, ignore it
        newline(),
        many1((id(), usage(), newline()).map(|(id, usage, _)| QuotaStatOsd { id, usage })),
    )
        .skip(optional(newline()))
        .skip(optional(eof()))
        .map(|(_, _, _, x)| x)
}

#[derive(Debug)]
pub(crate) enum QMTStat {
    Usr(Vec<QuotaStat>),
    Prj(Vec<QuotaStat>),
    Grp(Vec<QuotaStat>),
}

pub(crate) fn qmt_stat<I>() -> impl Parser<I, Output = (Param, QMTStat)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        string("glb-"),
        choice((
            (param(USR_QUOTAS), quota_stats().map(QMTStat::Usr)),
            (param(PRJ_QUOTAS), quota_stats().map(QMTStat::Prj)),
            (param(GRP_QUOTAS), quota_stats().map(QMTStat::Grp)),
        )),
    )
        .map(|(_, param)| (param))
}
pub(crate) fn qmt_parse<I>() -> impl Parser<I, Output = Record>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (qmt_target(), qmt_stat())
        .map(
            |((target, Target(manager), Target(pool)), (param, value))| match value {
                QMTStat::Usr(stats) => TargetStats::QuotaStats(TargetQuotaStat {
                    pool,
                    manager,
                    target,
                    param,
                    value: QuotaStats {
                        kind: QuotaKind::Usr,
                        stats,
                    },
                }),
                QMTStat::Prj(stats) => TargetStats::QuotaStats(TargetQuotaStat {
                    pool,
                    manager,
                    target,
                    param,
                    value: QuotaStats {
                        kind: QuotaKind::Prj,
                        stats,
                    },
                }),
                QMTStat::Grp(stats) => TargetStats::QuotaStats(TargetQuotaStat {
                    pool,
                    manager,
                    target,
                    param,
                    value: QuotaStats {
                        kind: QuotaKind::Grp,
                        stats,
                    },
                }),
            },
        )
        .map(Record::Target)
        .message("while parsing qmt")
}

#[cfg(test)]
mod tests {
    use crate::quota::quota_parser::{params, quota_stats, quota_stats_osd};
    use combine::Parser as _;

    #[test]
    fn test_qmt_params() {
        assert_eq!(
            params(),
            vec![
                "qmt.*.*.glb-usr".to_string(),
                "qmt.*.*.glb-prj".to_string(),
                "qmt.*.*.glb-grp".to_string(),
            ]
        )
    }

    #[test]
    fn test_parse_stats() {
        let x = r#"
global_pool0_dt_usr
- id:      0
  limits:  { hard:                    0, soft:                    0, granted:                    0, time:               604800 }
- id:      1337
  limits:  { hard:               309200, soft:               307200, granted:              1025032, time:           1687277628 }
  "#;

        insta::assert_debug_snapshot!(quota_stats().parse(x).map(|o| o.0).unwrap(), @r###"
        [
            QuotaStat {
                id: 0,
                limits: QuotaStatLimits {
                    hard: 0,
                    soft: 0,
                    granted: 0,
                    time: 604800,
                },
            },
            QuotaStat {
                id: 1337,
                limits: QuotaStatLimits {
                    hard: 309200,
                    soft: 307200,
                    granted: 1025032,
                    time: 1687277628,
                },
            },
        ]
        "###);
    }

    #[test]
    fn test_parse_stats_osd() {
        let x = r#"
usr_accounting:
- id:      0
  usage:   { inodes:              2416855, kbytes:             10214128 }
- id:      23
  usage:   { inodes:              241123355, kbytes:             102141328 }
  "#;

        insta::assert_debug_snapshot!(quota_stats_osd().parse(x).map(|o| o.0).unwrap(), @r###"
        [
            QuotaStatOsd {
                id: 0,
                usage: QuotaStatUsage {
                    inodes: 2416855,
                    kbytes: 10214128,
                },
            },
            QuotaStatOsd {
                id: 23,
                usage: QuotaStatUsage {
                    inodes: 241123355,
                    kbytes: 102141328,
                },
            },
        ]
        "###);
    }
}

// Copyright (c) 2024 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    QuotaKind, QuotaStat, QuotaStatOsd, QuotaStats, TargetQuotaStat,
    base_parsers::{param, period, target},
    types::{Param, Record, Target, TargetStats},
};
use combine::{
    Parser, attempt, choice, eof,
    error::{ParseError, StreamError},
    optional,
    parser::{
        char::{alpha_num, newline, string},
        repeat::take_until,
    },
    stream::{Stream, StreamErrorFor},
    token,
};

pub(crate) const USR_QUOTAS: &str = "usr";
pub(crate) const PRJ_QUOTAS: &str = "prj";
pub(crate) const GRP_QUOTAS: &str = "grp";

pub mod w {
    use crate::{
        Param, QuotaKind, QuotaStat, QuotaStatLimits, QuotaStatOsd, QuotaStats, Record,
        TargetQuotaStat,
        quota::{
            QMT,
            quota_parser::{GRP_QUOTAS, PRJ_QUOTAS, QMTStat, USR_QUOTAS},
        },
        types::{Target, TargetStats},
    };
    use winnow::{
        ModalResult,
        ascii::{dec_uint, multispace0, newline},
        combinator::{alt, delimited, opt, preceded, repeat, separated_pair, seq, terminated},
        prelude::*,
        stream::AsChar,
        token::{take_till, take_while},
    };

    /// Parses a target name
    fn target(input: &mut &str) -> ModalResult<Target> {
        take_while(1.., |c: char| {
            AsChar::is_alphanum(c) || c == '_' || c == '-'
        })
        .map(|s: &str| Target(s.into()))
        .parse_next(input)
    }

    /// Parses a target name
    fn qmt_pool(input: &mut &str) -> ModalResult<(Target, Target)> {
        (
            alt((
                terminated("md", "-").value(Target("md".into())),
                terminated("dt", "-").value(Target("dt".into())),
            )),
            target,
        )
            .parse_next(input)
    }

    /// Parses a target
    fn qmt_target(input: &mut &str) -> ModalResult<(Target, Target, Target)> {
        (terminated(target, "."), terminated(qmt_pool, "."))
            .map(|(target, (manager, pool))| (target, manager, pool))
            .parse_next(input)
    }

    fn id(input: &mut &str) -> ModalResult<u32> {
        delimited(("- id:", multispace0), dec_uint, newline).parse_next(input)
    }

    fn limit(input: &mut &str) -> ModalResult<QuotaStatLimits> {
        seq! {QuotaStatLimits {
            _: (multispace0, "limits:", multispace0, "{", multispace0),
            _: ("hard:", multispace0),
            hard: dec_uint,
            _: (", soft:", multispace0),
            soft: dec_uint,
            _: (", granted:", multispace0),
            granted: dec_uint,
            _: (", time:", multispace0),
            time: dec_uint,
            _: (multispace0, "}")
        }
        }
        .parse_next(input)
    }

    fn quota_stats(input: &mut &str) -> ModalResult<Vec<QuotaStat>> {
        preceded(
            (opt(newline), take_till(1.., AsChar::is_newline), newline),
            repeat(
                1..,
                terminated((id, limit), multispace0).map(|(id, limits)| QuotaStat {
                    id: id as u64,
                    limits,
                }),
            ),
        )
        .parse_next(input)
    }

    fn quota_stats_osd(input: &mut &str) -> ModalResult<Vec<QuotaStatOsd>> {
        delimited(
            (opt(newline), take_till(1.., AsChar::is_newline), newline),
            take_till(1.., AsChar::is_newline)
                .try_map(|s| serde_yaml::from_str::<Vec<QuotaStatOsd>>(s)),
            multispace0,
        )
        .parse_next(input)
    }

    fn qmt_stat(input: &mut &str) -> ModalResult<(Param, QMTStat)> {
        delimited(
            "glb-",
            alt((
                separated_pair(
                    USR_QUOTAS.map(|s: &str| Param(s.into())),
                    "=",
                    quota_stats.map(QMTStat::Usr),
                ),
                separated_pair(
                    PRJ_QUOTAS.map(|s: &str| Param(s.into())),
                    "=",
                    quota_stats.map(QMTStat::Prj),
                ),
                separated_pair(
                    GRP_QUOTAS.map(|s: &str| Param(s.into())),
                    "=",
                    quota_stats.map(QMTStat::Grp),
                ),
            )),
            multispace0,
        )
        .parse_next(input)
    }

    pub fn parse(input: &mut &str) -> ModalResult<Record> {
        preceded((QMT, "."), (qmt_target, qmt_stat))
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
            .parse_next(input)
    }

    pub fn parse_all(input: &mut &str) -> ModalResult<Vec<Record>> {
        repeat(1.., parse).parse_next(input)
    }
}

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

pub(crate) fn quota_stats<I>() -> impl Parser<I, Output = Vec<QuotaStat>>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        optional(newline()), // If quota stats are present, the whole yaml blob will start on a newline
        take_until::<Vec<_>, _, _>(newline()), // But yaml header might not be indented, ignore it
        newline(),
        take_until(attempt((newline(), alpha_num()).map(drop).or(eof()))),
    )
        .skip(optional(newline()))
        .skip(optional(eof()))
        .and_then(|(_, _, _, x): (_, _, _, String)| {
            serde_yaml::from_str::<Vec<QuotaStat>>(&x).map_err(StreamErrorFor::<I>::other)
        })
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
        take_until(attempt((newline(), alpha_num()).map(drop).or(eof()))),
    )
        .skip(optional(newline()))
        .skip(optional(eof()))
        .and_then(|(_, _, _, x): (_, _, _, String)| {
            serde_yaml::from_str::<Vec<QuotaStatOsd>>(&x).map_err(StreamErrorFor::<I>::other)
        })
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
    use crate::{QuotaStat, QuotaStatLimits, quota::params};

    #[test]
    fn test_qmt_params() {
        assert_eq!(
            params().into_iter().map(String::from).collect::<Vec<_>>(),
            vec![
                "qmt.*.*.glb-usr".to_string(),
                "qmt.*.*.glb-prj".to_string(),
                "qmt.*.*.glb-grp".to_string(),
            ]
        )
    }

    #[test]
    fn test_yaml_deserialize() {
        let x = r#"
- id:      0
  limits:  { hard:                    0, soft:                    0, granted:                    0, time:               604800 }
- id:      1337
  limits:  { hard:               309200, soft:               307200, granted:              1025032, time:           1687277628 }"#;

        let expected = vec![
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
        ];

        assert_eq!(serde_yaml::from_str::<Vec<QuotaStat>>(x).unwrap(), expected)
    }
}

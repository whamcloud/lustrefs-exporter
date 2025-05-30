// Copyright (c) 2024 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    QuotaKind, QuotaStat, QuotaStatOsd, QuotaStats, TargetQuotaStat,
    base_parsers::{param, period, target},
    quota::QMT,
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
    let not_supported_parser = attempt(
        string("not supported")
            .skip(optional(newline()))
            .map(|_| Vec::new()),
    );

    let yaml_parser = (
        optional(newline()), // If quota stats are present, the whole yaml blob will start on a newline
        take_until::<Vec<_>, _, _>(newline()), // But yaml header might not be indented, ignore it
        newline(),
        take_until(attempt((newline(), alpha_num()).map(drop).or(eof()))),
    )
        .skip(optional(newline()))
        .skip(optional(eof()))
        .and_then(|(_, _, _, x): (_, _, _, String)| {
            serde_yaml::from_str::<Vec<QuotaStatOsd>>(&x).map_err(StreamErrorFor::<I>::other)
        });
    not_supported_parser.or(yaml_parser)
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
    use crate::{QuotaStat, QuotaStatLimits};

    use super::*;

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

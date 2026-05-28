// Copyright (c) 2026 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    base_parsers::{digits, param, period, target},
    types::{Param, Record, Target, TargetStat, TargetStats, TargetVariant},
};
use combine::{
    Parser, attempt, choice,
    error::{ParseError, StreamError},
    parser::char::{newline, string},
    stream::{Stream, StreamErrorFor},
};

pub(crate) const ACTIVE: &str = "active";
pub(crate) const MAX_CREATE_COUNT: &str = "max_create_count";

pub(crate) fn params() -> Vec<String> {
    vec![
        format!("osp.*.{ACTIVE}"),
        format!("osp.*.{MAX_CREATE_COUNT}"),
    ]
}

/// Parses the osp target name and extracts the target variant.
///
/// The osp param format is: `osp.<fsname>-<TARGET>-osc-<MDT>.param=value`
/// For example: `osp.fs-OST0000-osc-MDT0000.active=1`
///
/// We extract `fs-OST0000-osc-MDT0000` as the full target name,
/// and determine the variant from the OST/MDT part.
fn target_and_variant<I>() -> impl Parser<I, Output = (Target, TargetVariant)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        attempt(string("osp")).skip(period()),
        target().skip(period()),
    )
        .and_then(move |(_, x)| -> Result<_, _> {
            // The target name is something like "fs-OST0000-osc-MDT0000"
            // We need to determine the variant from the target part.
            // For osp, the target contains an OST reference (e.g., fs-OST0000-osc-MDT0000)
            // We look for "OST" or "MDT" in the target name to determine the variant.
            let target_lower = x.0.to_lowercase();

            let variant = if target_lower.contains("ost") {
                TargetVariant::Ost
            } else if target_lower.contains("mdt") {
                TargetVariant::Mdt
            } else {
                return Err(StreamErrorFor::<I>::other(
                    crate::LustreCollectorError::ConversionError(format!(
                        "Could not determine target variant from osp target: {}",
                        x.0
                    )),
                ));
            };

            Ok((x, variant))
        })
        .message("while parsing osp target_and_variant")
}

#[derive(Debug)]
enum OspStat {
    /// Whether the OSP target is active (1) or deactivated (0)
    Active(u64),
    /// Maximum object creation count (0 means disabled)
    MaxCreateCount(u64),
}

fn osp_stat<I>() -> impl Parser<I, Output = (Param, OspStat)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    choice((
        (param(ACTIVE), digits().skip(newline()).map(OspStat::Active))
            .message("while parsing osp active"),
        (
            param(MAX_CREATE_COUNT),
            digits().skip(newline()).map(OspStat::MaxCreateCount),
        )
            .message("while parsing osp max_create_count"),
    ))
}

pub(crate) fn parse<I>() -> impl Parser<I, Output = Record>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (target_and_variant(), osp_stat())
        .map(|((target, kind), (param, stat))| match stat {
            OspStat::Active(value) => TargetStats::OspActive(TargetStat {
                kind,
                target,
                param,
                value,
            }),
            OspStat::MaxCreateCount(value) => TargetStats::OspMaxCreateCount(TargetStat {
                kind,
                target,
                param,
                value,
            }),
        })
        .map(Record::Target)
        .message("while parsing osp")
}

#[cfg(test)]
mod tests {
    use combine::{EasyParser, many};
    use insta::assert_debug_snapshot;

    use super::*;

    #[test]
    fn test_osp_stats() {
        static FIXTURE: &str = include_str!("fixtures/osp.txt");

        let result = many::<Vec<_>, _, _>(parse())
            .easy_parse(FIXTURE)
            .map_err(|err| err.map_position(|p| p.translate_position(FIXTURE)))
            .unwrap();

        assert_debug_snapshot!(result);
    }

    #[test]
    fn test_osp_target_name() {
        let result = target_and_variant().parse("osp.tesfs-OST0000-osc-MDT0000.active=1\n");

        assert_eq!(
            result,
            Ok((
                (
                    Target("tesfs-OST0000-osc-MDT0000".to_string()),
                    TargetVariant::Ost
                ),
                "active=1\n"
            ))
        );
    }

    #[test]
    fn test_osp_mdt_target_name() {
        let result = target_and_variant().parse("osp.tesfs-MDT0001-osp-MDT0000.active=1\n");

        assert_eq!(
            result,
            Ok((
                (
                    Target("tesfs-MDT0001-osp-MDT0000".to_string()),
                    TargetVariant::Mdt
                ),
                "active=1\n"
            ))
        );
    }
}

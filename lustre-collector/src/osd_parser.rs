// Copyright (c) 2021 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    base_parsers::{digits, param, period, target, till_newline, till_period},
    brw_stats_parser::brw_stats,
    quota::quota_parser::quota_stats_osd,
    types::{BrwStats, Param, Record, Target, TargetStat, TargetStats, TargetVariant},
    QuotaKind, QuotaStatsOsd,
};
use combine::{
    attempt, choice,
    error::{ParseError, StreamError},
    parser::char::{newline, string},
    stream::{Stream, StreamErrorFor},
    Parser,
};

pub(crate) const FILES_FREE: &str = "filesfree";
pub(crate) const FILES_TOTAL: &str = "filestotal";
pub(crate) const KBYTES_AVAIL: &str = "kbytesavail";
pub(crate) const KBYTES_FREE: &str = "kbytesfree";
pub(crate) const KBYTES_TOTAL: &str = "kbytestotal";
pub(crate) const FS_TYPE: &str = "fstype";

pub(crate) const BRW_STATS: &str = "brw_stats";

pub(crate) const QUOTA_ACCT_GRP: &str = "quota_slave.acct_group";
pub(crate) const QUOTA_ACCT_USR: &str = "quota_slave.acct_user";
pub(crate) const QUOTA_ACCT_PRJ: &str = "quota_slave.acct_project";

pub(crate) fn params() -> Vec<String> {
    vec![
        format!("osd-*.*.{FILES_FREE}"),
        format!("osd-*.*.{FILES_TOTAL}"),
        format!("osd-*.*.{FS_TYPE}"),
        format!("osd-*.*.{KBYTES_AVAIL}"),
        format!("osd-*.*.{KBYTES_FREE}"),
        format!("osd-*.*.{KBYTES_TOTAL}"),
        format!("osd-*.*.{BRW_STATS}"),
        format!("osd-*.*.{QUOTA_ACCT_GRP}"),
        format!("osd-*.*.{QUOTA_ACCT_USR}"),
        format!("osd-*.*.{QUOTA_ACCT_PRJ}"),
    ]
}

#[derive(Debug)]
enum OsdStat {
    /// Available inodes
    FilesFree(u64),
    /// Total inodes
    FilesTotal(u64),
    /// Type of target
    FsType(String),
    /// Available disk space
    KBytesAvail(u64),
    /// Free disk space
    KBytesFree(u64),
    /// Total disk space
    KBytesTotal(u64),
    BrwStats(Vec<BrwStats>),
    QuotaStats(QuotaStatsOsd),
}

fn target_and_variant<I>() -> impl Parser<I, Output = (Target, TargetVariant)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        attempt(string("osd-").skip(till_period())).skip(period()),
        target().skip(period()),
    )
        .and_then(move |(_, x)| -> Result<_, _> {
            let variant = match (&x).try_into() {
                Ok(x) => x,
                Err(e) => return Err(StreamErrorFor::<I>::other(e)),
            };

            Ok((x, variant))
        })
        .message("while parsing target_and_variant")
}

fn osd_stat<I>() -> impl Parser<I, Output = (Param, OsdStat)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    choice((
        (param(BRW_STATS), brw_stats().map(OsdStat::BrwStats)),
        (
            param(FILES_FREE),
            digits().skip(newline()).map(OsdStat::FilesFree),
        )
            .message("while parsing files free"),
        (
            param(FILES_TOTAL),
            digits().skip(newline()).map(OsdStat::FilesTotal),
        )
            .message("while parsing files total"),
        (
            param(FS_TYPE),
            till_newline().skip(newline()).map(OsdStat::FsType),
        )
            .message("while parsing fs type"),
        (
            param(KBYTES_AVAIL),
            digits().skip(newline()).map(OsdStat::KBytesAvail),
        ),
        (
            param(KBYTES_FREE),
            digits().skip(newline()).map(OsdStat::KBytesFree),
        ),
        (
            param(KBYTES_TOTAL),
            digits().skip(newline()).map(OsdStat::KBytesTotal),
        ),
        (
            param(QUOTA_ACCT_GRP),
            quota_stats_osd().map(|stats| {
                OsdStat::QuotaStats(QuotaStatsOsd {
                    kind: QuotaKind::Grp,
                    stats,
                })
            }),
        ),
        (
            param(QUOTA_ACCT_PRJ),
            quota_stats_osd().map(|stats| {
                OsdStat::QuotaStats(QuotaStatsOsd {
                    kind: QuotaKind::Prj,
                    stats,
                })
            }),
        ),
        (
            param(QUOTA_ACCT_USR),
            quota_stats_osd().map(|stats| {
                OsdStat::QuotaStats(QuotaStatsOsd {
                    kind: QuotaKind::Usr,
                    stats,
                })
            }),
        ),
    ))
}

pub(crate) fn parse<'a, I>() -> impl Parser<I, Output = Record<'a>>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (target_and_variant(), osd_stat())
        .map(|((target, kind), (param, stat))| match stat {
            OsdStat::FilesFree(value) => TargetStats::FilesFree(TargetStat {
                kind,
                target,
                param,
                value,
            }),
            OsdStat::FilesTotal(value) => TargetStats::FilesTotal(TargetStat {
                kind,
                target,
                param,
                value,
            }),
            OsdStat::FsType(value) => TargetStats::FsType(TargetStat {
                kind,
                target,
                param,
                value,
            }),
            OsdStat::KBytesAvail(value) => TargetStats::KBytesAvail(TargetStat {
                kind,
                target,
                param,
                value,
            }),
            OsdStat::KBytesFree(value) => TargetStats::KBytesFree(TargetStat {
                kind,
                target,
                param,
                value,
            }),
            OsdStat::KBytesTotal(value) => TargetStats::KBytesTotal(TargetStat {
                kind,
                target,
                param,
                value,
            }),
            OsdStat::BrwStats(value) => TargetStats::BrwStats(TargetStat {
                kind,
                target,
                param,
                value,
            }),
            OsdStat::QuotaStats(value) => TargetStats::QuotaStatsOsd(TargetStat {
                kind,
                target,
                param,
                value,
            }),
        })
        .map(Record::Target)
        .message("while parsing osd")
}

#[cfg(test)]
mod tests {
    use combine::{many, EasyParser};
    use insta::assert_debug_snapshot;

    use super::*;

    #[test]
    fn test_osd_stats() {
        static FIXTURE: &str = include_str!("fixtures/osd.txt");

        let result = many::<Vec<_>, _, _>(parse())
            .easy_parse(FIXTURE)
            .map_err(|err| err.map_position(|p| p.translate_position(FIXTURE)))
            .unwrap();

        assert_debug_snapshot!(result);
    }

    #[test]
    fn test_osd_active_stats() {
        static FIXTURE: &str = include_str!("fixtures/osd_active.txt");

        let result = many::<Vec<_>, _, _>(parse())
            .easy_parse(FIXTURE)
            .map_err(|err| err.map_position(|p| p.translate_position(FIXTURE)))
            .unwrap();

        assert_debug_snapshot!(result);
    }
}

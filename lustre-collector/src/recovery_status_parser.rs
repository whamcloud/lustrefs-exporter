// Copyright (c) 2021 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use std::iter;

use crate::{
    base_parsers::{digits, param, period, target, till_newline},
    types::{Param, Record, RecoveryStatus, Target, TargetStat, TargetStats, TargetVariant},
};
use combine::{
    ParseError, Parser, attempt, choice, eof, many, many1, one_of, optional,
    parser::{
        char::{alpha_num, newline, spaces, string},
        repeat::{skip_until, take_until},
    },
    stream::Stream,
    token,
};

pub const RECOVERY_STATUS: &str = "recovery_status";

pub fn params() -> Vec<String> {
    vec![
        format!("obdfilter.*OST*.{}", RECOVERY_STATUS),
        format!("mdt.*MDT*.{}", RECOVERY_STATUS),
    ]
}

fn ost_or_mdt<I>() -> impl Parser<I, Output = TargetVariant>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    string("obdfilter")
        .map(|_| TargetVariant::Ost)
        .or(string("mdt").map(|_| TargetVariant::Mdt))
        .message("while parsing target_name")
}

/// Parses the name and kind of a target
fn target_info<I>() -> impl Parser<I, Output = (TargetVariant, Target, Param)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        ost_or_mdt().skip(period()),
        target().skip(period()),
        param(RECOVERY_STATUS),
    )
        .map(|(variant, x, param)| (variant, x, param))
        .message("while parsing target_name")
}

fn status_line<I>() -> impl Parser<I, Output = RecoveryStatus>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        string("status").skip(optional(token(':'))),
        spaces(),
        take_until(newline().map(drop).or(eof())),
    )
        .map(|(_, _, x): (_, _, String)| match x.as_ref() {
            "COMPLETE" => RecoveryStatus::Complete,
            "INACTIVE" => RecoveryStatus::Inactive,
            "WAITING" => RecoveryStatus::Waiting,
            "WAITING_FOR_CLIENTS" => RecoveryStatus::WaitingForClients,
            "RECOVERING" => RecoveryStatus::Recovering,
            _ => RecoveryStatus::Unknown,
        })
}

/// Parses a client line containing the completed and a optional value for the total count (e.g., "completed: 5/10").
fn clients_line<I>(x: &'static str) -> impl Parser<I, Output = (u64, Option<u64>)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        attempt(string(x).skip(optional(token(':')))),
        spaces(),
        digits(),
        optional((token('/'), digits())),
        optional(newline().map(drop).or(eof())),
    )
        .map(|(_, _, x, y, _): (_, _, u64, Option<(_, u64)>, _)| (x, y.map(|(_, v)| v)))
}

#[derive(Debug)]
enum RecoveryStat {
    Completed(u64),
    Connected(u64),
    Evicted(u64),
    RecoveryDuration(u64),
    Status(RecoveryStatus),
    TimeRemaining(u64),
    Total(Option<u64>),
}

pub struct StatName(pub String);

/// Parses a stat name
pub(crate) fn stat_name<I>() -> impl Parser<I, Output = StatName>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    many1(alpha_num().or(one_of("_-".chars()))).map(StatName)
}

/// Parses all the simple recovery stats for a target, which are just plain u64 values
fn simple_client_stat<I>(
    name: &'static str,
    constructor: fn(u64) -> RecoveryStat,
) -> impl Parser<I, Output = Vec<RecoveryStat>>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    clients_line(name)
        .skip(optional(newline()))
        .map(move |(x, _)| vec![constructor(x)])
}

fn target_recovery_stats<I>() -> impl Parser<I, Output = Vec<RecoveryStat>>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    many(choice((
        status_line()
            .skip(optional(newline()))
            .map(|x| vec![RecoveryStat::Status(x)]),
        simple_client_stat("recovery_duration", RecoveryStat::RecoveryDuration),
        simple_client_stat("completed_clients", RecoveryStat::Completed),
        simple_client_stat("time_remaining", RecoveryStat::TimeRemaining),
        simple_client_stat("evicted_clients", RecoveryStat::Evicted),
        clients_line("connected_clients")
            .skip(optional(newline()))
            .map(|(x, y)| {
                iter::once(RecoveryStat::Connected(x))
                    .chain(
                        y.map(|total| vec![RecoveryStat::Total(Some(total))])
                            .unwrap_or_default(),
                    )
                    .collect()
            }),
        // This will ignore line/field we don't care
        attempt((stat_name(), token(':'), till_newline().skip(newline()))).map(|_| vec![]),
    )))
    .map(|xs: Vec<_>| xs.into_iter().flatten().collect())
}

fn target_status<I>() -> impl Parser<I, Output = Vec<TargetStats>>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        target_info().skip(optional(newline())),
        target_recovery_stats(),
    )
        .map(|((kind, target, param), values)| {
            values
                .iter()
                .map(|value| match value {
                    RecoveryStat::Status(value) => TargetStats::RecoveryStatus(TargetStat {
                        kind,
                        param: param.clone(),
                        target: target.clone(),
                        value: *value,
                    }),
                    RecoveryStat::Completed(value) => {
                        TargetStats::RecoveryCompletedClients(TargetStat {
                            kind,
                            param: param.clone(),
                            target: target.clone(),
                            value: *value,
                        })
                    }
                    RecoveryStat::Connected(value) => {
                        TargetStats::RecoveryConnectedClients(TargetStat {
                            kind,
                            param: param.clone(),
                            target: target.clone(),
                            value: *value,
                        })
                    }
                    RecoveryStat::Evicted(value) => {
                        TargetStats::RecoveryEvictedClients(TargetStat {
                            kind,
                            param: param.clone(),
                            target: target.clone(),
                            value: *value,
                        })
                    }
                    RecoveryStat::RecoveryDuration(value) => {
                        TargetStats::RecoveryDuration(TargetStat {
                            kind,
                            param: param.clone(),
                            target: target.clone(),
                            value: *value,
                        })
                    }
                    RecoveryStat::TimeRemaining(value) => {
                        TargetStats::RecoveryTimeRemaining(TargetStat {
                            kind,
                            param: param.clone(),
                            target: target.clone(),
                            value: *value,
                        })
                    }
                    RecoveryStat::Total(value) => TargetStats::RecoveryTotalClients(TargetStat {
                        kind,
                        param: param.clone(),
                        target: target.clone(),
                        value: value.unwrap_or(0),
                    }),
                })
                .collect()
        })
}

pub fn parse<I>() -> impl Parser<I, Output = Vec<Record>>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        target_status(),
        skip_until(attempt(ost_or_mdt().map(drop)).or(eof())),
    )
        .map(|(x, _)| x.into_iter().map(Record::Target).collect())
}

#[cfg(test)]
mod tests {
    use crate::parser::parse;
    use crate::recovery_status_parser::{clients_line, target_recovery_stats};
    use combine::{Parser, parser::EasyParser, stream::position};

    #[test]
    fn test_multiple() {
        let x = include_str!("fixtures/recovery-multiple.txt");

        let (records, _): (Vec<_>, _) = parse().easy_parse(position::Stream::new(x)).unwrap();

        insta::assert_debug_snapshot!(records);
    }

    #[test]
    fn test_multiple_recovering() {
        let x = include_str!("fixtures/recovery-multiple-recovering.txt");

        let (records, _): (Vec<_>, _) = parse().easy_parse(position::Stream::new(x)).unwrap();

        insta::assert_debug_snapshot!(records);
    }

    #[test]
    fn test_empty_input() {
        let (records, _): (Vec<_>, _) = parse().easy_parse(position::Stream::new("")).unwrap();

        assert_eq!(records, vec![]);
    }

    #[test]
    fn test_waiting_for_clients() {
        let x = include_str!("fixtures/recovery-waiting-for-clients.txt");

        let (records, _): (Vec<_>, _) = parse().easy_parse(position::Stream::new(x)).unwrap();

        insta::assert_debug_snapshot!(records);
    }

    #[test]
    fn test_clients_line() {
        let result = clients_line("completed_clients").parse("completed_clients: 3/7\n");
        assert_eq!(result, Ok(((3, Some(7)), "")));
        let result = clients_line("connected_clients").parse("connected_clients: 3/7\n");
        assert_eq!(result, Ok(((3, Some(7)), "")));
        let result = clients_line("completed_clients").parse("completed_clients: 3\n");
        assert_eq!(result, Ok(((3, None), "")));
    }

    #[test]
    fn test_target_recovery_stats() {
        let x = r#"status: COMPLETE
recovery_start: 1620410016
recovery_duration: 150
completed_clients: 4/8
replayed_requests: 0
last_transno: 4294967296
VBR: ENABLED
IR: ENABLED
"#;

        let (records, _): (Vec<_>, _) = target_recovery_stats().parse(x).unwrap();

        insta::assert_debug_snapshot!(records, @r"
        [
            Status(
                Complete,
            ),
            RecoveryDuration(
                150,
            ),
            Completed(
                4,
            ),
        ]
        ");
    }

    #[test]
    fn test_target_recovery_stats2() {
        let x = r#"status: RECOVERING
recovery_start: 1620920843
time_remaining: 119
connected_clients: 3/7
req_replay_clients: 0
lock_repay_clients: 0
completed_clients: 3
"#;

        let (records, _): (Vec<_>, _) = target_recovery_stats().parse(x).unwrap();

        insta::assert_debug_snapshot!(records, @r"
        [
            Status(
                Recovering,
            ),
            TimeRemaining(
                119,
            ),
            Connected(
                3,
            ),
            Total(
                Some(
                    7,
                ),
            ),
            Completed(
                3,
            ),
        ]
        ");
    }
}

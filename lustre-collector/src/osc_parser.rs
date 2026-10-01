// Copyright (c) 2026 DDN. All rights reserved.
// Copyright (c) 2026 Google LLC
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

//! `osc.<controller>.<param>`; `io_latency_stats` (DDN 2.14 builds, upstream
//! 2.17.51+) shares the `osd-*` parser.

use crate::{
    base_parsers::{controller, digits, param},
    io_latency_stats_parser::io_latency_stats,
    key_value_parser::key_values,
    lockless_stats_parser::lockless_stats,
    rpc_stats_parser::rpc_stats,
    stats_parser::stats,
    time::StatsHeader,
    types::{
        BrwStats, ControllerStat, ControllerStats, ControllerVariant, KeyValue, LocklessStats,
        OscState, Param, Record, RpcStats, Stat, TimedControllerStat,
    },
};
use combine::{
    Parser, attempt, choice,
    error::{ParseError, StreamError},
    many1,
    parser::char::{newline, string},
    parser::token::satisfy,
    stream::{Stream, StreamErrorFor},
};

pub(crate) const OSC: &str = "osc";
pub(crate) const STATE: &str = "state";
pub(crate) const STATS: &str = "stats";
pub(crate) const RPC_STATS: &str = "rpc_stats";
pub(crate) const IO_LATENCY_STATS: &str = "io_latency_stats";
pub(crate) const OSC_STATS: &str = "osc_stats";
pub(crate) const UNSTABLE_STATS: &str = "unstable_stats";
pub(crate) const STATS_COMPR: &str = "stats_compr";
pub(crate) const CUR_GRANT_BYTES: &str = "cur_grant_bytes";
pub(crate) const CUR_DIRTY_BYTES: &str = "cur_dirty_bytes";

pub(crate) fn params() -> Vec<String> {
    [
        STATE,
        STATS,
        RPC_STATS,
        IO_LATENCY_STATS,
        OSC_STATS,
        UNSTABLE_STATS,
        STATS_COMPR,
        CUR_GRANT_BYTES,
        CUR_DIRTY_BYTES,
    ]
    .into_iter()
    .map(|x| format!("{OSC}.*.{x}"))
    .collect()
}

enum OscStat {
    State(OscState),
    Stats(StatsHeader, Vec<Stat>),
    RpcStats(StatsHeader, RpcStats),
    IoLatencyStats(StatsHeader, Vec<BrwStats>),
    LocklessStats(StatsHeader, LocklessStats),
    UnstableStats(Vec<KeyValue>),
    CompressionStats(Vec<KeyValue>),
    CurGrantBytes(u64),
    CurDirtyBytes(u64),
}

fn read_until_next_param<I>() -> impl Parser<I, Output = String>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    let read_line = many1::<String, _, _>(satisfy(|c| c != '\n'))
        .skip(newline())
        .map(|s: String| format!("{}\n", s));

    combine::parser::repeat::repeat_until(
        read_line,
        combine::look_ahead(
            attempt(
                combine::parser::repeat::skip_many(satisfy(|c: char| c != '\n' && c != '='))
                    .with(string("=")),
            )
            .map(|_| ()),
        )
        .or(combine::eof().map(|_| ())),
    )
    .map(|lines: Vec<String>| lines.join(""))
}

fn osc_state<I>() -> impl Parser<I, Output = OscState>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    newline()
        .with(read_until_next_param())
        .and_then(|yaml_str: String| -> Result<OscState, StreamErrorFor<I>> {
            yaml_serde::from_str(&yaml_str).map_err(StreamErrorFor::<I>::other)
        })
        .message("while parsing osc state")
}

fn osc_stat<I>() -> impl Parser<I, Output = (Param, OscStat)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    choice((
        (param(STATE), osc_state().map(OscStat::State)),
        (
            param(STATS_COMPR),
            key_values().map(OscStat::CompressionStats),
        ),
        (param(STATS), stats().map(|(h, v)| OscStat::Stats(h, v))),
        (
            param(RPC_STATS),
            rpc_stats().map(|(h, v)| OscStat::RpcStats(h, v)),
        ),
        (
            param(IO_LATENCY_STATS),
            io_latency_stats().map(|(h, v)| OscStat::IoLatencyStats(h, v)),
        ),
        (
            param(OSC_STATS),
            lockless_stats().map(|(h, v)| OscStat::LocklessStats(h, v)),
        ),
        (
            param(UNSTABLE_STATS),
            key_values().map(OscStat::UnstableStats),
        ),
        (
            param(CUR_GRANT_BYTES),
            digits().skip(newline()).map(OscStat::CurGrantBytes),
        ),
        (
            param(CUR_DIRTY_BYTES),
            digits().skip(newline()).map(OscStat::CurDirtyBytes),
        ),
    ))
    .message("while parsing osc_stat")
}

pub(crate) fn parse<I>() -> impl Parser<I, Output = Record>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (controller(OSC), osc_stat())
        .map(|(controller, (param, stat))| {
            let kind = ControllerVariant::Osc;

            match stat {
                OscStat::State(value) => ControllerStats::OscState(ControllerStat {
                    kind,
                    param,
                    controller,
                    value,
                }),
                OscStat::Stats(header, value) => ControllerStats::Stats(TimedControllerStat {
                    kind,
                    param,
                    controller,
                    value,
                    header,
                }),
                OscStat::RpcStats(header, value) => {
                    ControllerStats::RpcStats(TimedControllerStat {
                        kind,
                        param,
                        controller,
                        value,
                        header,
                    })
                }
                OscStat::IoLatencyStats(header, value) => {
                    ControllerStats::IoLatencyStats(TimedControllerStat {
                        kind,
                        param,
                        controller,
                        value,
                        header,
                    })
                }
                OscStat::LocklessStats(header, value) => {
                    ControllerStats::LocklessStats(TimedControllerStat {
                        kind,
                        param,
                        controller,
                        value,
                        header,
                    })
                }
                OscStat::UnstableStats(value) => ControllerStats::UnstableStats(ControllerStat {
                    kind,
                    param,
                    controller,
                    value,
                }),
                OscStat::CompressionStats(value) => {
                    ControllerStats::CompressionStats(ControllerStat {
                        kind,
                        param,
                        controller,
                        value,
                    })
                }
                OscStat::CurGrantBytes(value) => ControllerStats::CurGrantBytes(ControllerStat {
                    kind,
                    param,
                    controller,
                    value,
                }),
                OscStat::CurDirtyBytes(value) => ControllerStats::CurDirtyBytes(ControllerStat {
                    kind,
                    param,
                    controller,
                    value,
                }),
            }
        })
        .map(Record::Controller)
        .message("while parsing osc param")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ControllerState;
    use combine::EasyParser;

    #[test]
    fn test_parse_osc_state() {
        let input = r#"osc.fs-OST0000-osc-MDT0000.state=
current_state: FULL
state_history:
 - [ 1775627216, CONNECTING ]
 - [ 1775627216, FULL ]
"#;

        let result = parse().easy_parse(input);
        assert!(result.is_ok(), "Parse failed: {:?}", result);

        let (record, _) = result.unwrap();
        match record {
            Record::Controller(ControllerStats::OscState(stat)) => {
                assert_eq!(stat.controller.0, "fs-OST0000-osc-MDT0000");
                assert_eq!(stat.value.current_state, ControllerState::Full);
            }
            _ => panic!("Expected OscState record"),
        }
    }

    #[test]
    fn test_params() {
        assert_eq!(
            params(),
            vec![
                "osc.*.state",
                "osc.*.stats",
                "osc.*.rpc_stats",
                "osc.*.io_latency_stats",
                "osc.*.osc_stats",
                "osc.*.unstable_stats",
                "osc.*.stats_compr",
                "osc.*.cur_grant_bytes",
                "osc.*.cur_dirty_bytes",
            ]
        );
    }
}

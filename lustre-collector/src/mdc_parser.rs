// Copyright (c) 2026 Google LLC
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

//! `mdc.<controller>.{md_stats,stats,rpc_stats,mdc_stats}`.

use crate::{
    base_parsers::{controller, param},
    lockless_stats_parser::lockless_stats,
    rpc_stats_parser::rpc_stats,
    stats_parser::stats,
    time::StatsHeader,
    types::{
        ControllerStats, ControllerVariant, LocklessStats, Param, Record, RpcStats, Stat,
        TimedControllerStat,
    },
};
use combine::{Parser, choice, error::ParseError, stream::Stream};

pub(crate) const MDC: &str = "mdc";
pub(crate) const MD_STATS: &str = "md_stats";
pub(crate) const STATS: &str = "stats";
pub(crate) const RPC_STATS: &str = "rpc_stats";
pub(crate) const MDC_STATS: &str = "mdc_stats";

pub(crate) fn params() -> Vec<String> {
    [MD_STATS, STATS, RPC_STATS, MDC_STATS]
        .into_iter()
        .map(|x| format!("{MDC}.*.{x}"))
        .collect()
}

enum MdcStat {
    MdStats(StatsHeader, Vec<Stat>),
    Stats(StatsHeader, Vec<Stat>),
    RpcStats(StatsHeader, RpcStats),
    LocklessStats(StatsHeader, LocklessStats),
}

fn mdc_stat<I>() -> impl Parser<I, Output = (Param, MdcStat)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    choice((
        (
            param(MD_STATS),
            stats().map(|(h, v)| MdcStat::MdStats(h, v)),
        ),
        (
            param(MDC_STATS),
            lockless_stats().map(|(h, v)| MdcStat::LocklessStats(h, v)),
        ),
        (param(STATS), stats().map(|(h, v)| MdcStat::Stats(h, v))),
        (
            param(RPC_STATS),
            rpc_stats().map(|(h, v)| MdcStat::RpcStats(h, v)),
        ),
    ))
    .message("while parsing mdc_stat")
}

pub(crate) fn parse<I>() -> impl Parser<I, Output = Record>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (controller(MDC), mdc_stat())
        .map(|(controller, (param, value))| {
            let kind = ControllerVariant::Mdc;

            match value {
                MdcStat::MdStats(header, value) => ControllerStats::MdStats(TimedControllerStat {
                    kind,
                    param,
                    controller,
                    value,
                    header,
                }),
                MdcStat::Stats(header, value) => ControllerStats::Stats(TimedControllerStat {
                    kind,
                    param,
                    controller,
                    value,
                    header,
                }),
                MdcStat::RpcStats(header, value) => {
                    ControllerStats::RpcStats(TimedControllerStat {
                        kind,
                        param,
                        controller,
                        value,
                        header,
                    })
                }
                MdcStat::LocklessStats(header, value) => {
                    ControllerStats::LocklessStats(TimedControllerStat {
                        kind,
                        param,
                        controller,
                        value,
                        header,
                    })
                }
            }
        })
        .map(Record::Controller)
        .message("while parsing mdc")
}

#[cfg(test)]
mod tests {
    use super::*;
    use combine::{EasyParser, many};
    use insta::assert_debug_snapshot;

    #[test]
    fn params_are_requested_in_order() {
        assert_eq!(
            params(),
            [
                "mdc.*.md_stats",
                "mdc.*.stats",
                "mdc.*.rpc_stats",
                "mdc.*.mdc_stats"
            ]
        );
    }

    #[test]
    fn parses_md_stats_then_stats() {
        let input = "mdc.a361000-MDT0000-mdc-ffff94d28d903800.md_stats=\nsnapshot_time             1789952232.646146706 secs.nsecs\nstart_time                1787864516.486398220 secs.nsecs\nelapsed_time              2087716.159748486 secs.nsecs\nclose                     1024327 samples [reqs]\ncreate                    2 samples [reqs]\nenqueue                   45166 samples [reqs]\ngetattr                   4326 samples [reqs]\nintent_lock               1046326 samples [reqs]\nmdc.a361000-MDT0000-mdc-ffff94d28d903800.stats=\nsnapshot_time             1789952232.657219153 secs.nsecs\nstart_time                1787864516.486401920 secs.nsecs\nelapsed_time              2087716.170817233 secs.nsecs\nreq_waittime              2222271 samples [usecs] 101 189010 927538421 1313195185071\nreq_active                2222271 samples [reqs] 1 4 4224905 9948733\nmds_close                 1024327 samples [usecs] 103 189010 417563414 597054272656\nldlm_enqueue              1091492 samples [usecs] 105 165006 434824356 553257670099\nosc.a361000-OST0000-osc-ffff949bc0626000.cur_grant_bytes=8437760\n";

        let (records, rest): (Vec<Record>, _) = many(parse()).easy_parse(input).unwrap();

        assert_eq!(
            rest,
            "osc.a361000-OST0000-osc-ffff949bc0626000.cur_grant_bytes=8437760\n"
        );
        assert_eq!(records.len(), 2);

        assert_debug_snapshot!(records);
    }
}

// Copyright (c) 2021 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    base_parsers::{digits, string_to, till_newline},
    types::{NodeStat, Param, Record},
    LustreCollectorError, NodeStats,
};
use combine::{
    attempt, choice,
    error::ParseError,
    parser::EasyParser,
    parser::{
        char::{newline, spaces, string},
        repeat::take_until,
    },
    sep_end_by,
    stream::Stream,
    token, Parser,
};
use std::io;

pub fn parse_cpustats_output(output: &[u8]) -> Result<Vec<Record>, LustreCollectorError> {
    let output = std::str::from_utf8(output)?;

    let (stats, state) = parse_cpustats()
        .easy_parse(output)
        .map_err(|err| err.map_position(|p| p.translate_position(output)))?;

    let params = crate::parser::params().join(" ");

    if !state.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Content left in input buffer. Please run and supply output to support: `lctl get_param {params}`"),
        )
        .into());
    }

    Ok(stats)
}

fn parse_cpustats<'a, I>() -> impl Parser<I, Output = Vec<Record<'a>>>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (string("cpu").skip(spaces()), sep_end_by(digits(), spaces())).map(|(_, xs): (_, Vec<_>)| {
        vec![
            Record::Node(NodeStats::CpuTotal(NodeStat {
                param: Param("cpu_total".into()),
                value: xs.iter().take(6).sum(),
            })),
            Record::Node(NodeStats::CpuUser(NodeStat {
                param: Param("cpu_user".into()),
                value: xs.iter().take(1).sum(),
            })),
            Record::Node(NodeStats::CpuIowait(NodeStat {
                param: Param("cpu_iowait".into()),
                value: xs.get(4).cloned().unwrap_or_default(),
            })),
            Record::Node(NodeStats::CpuSystem(NodeStat {
                param: Param("cpu_system".into()),
                value: xs
                    .get(2)
                    .and_then(|x| {
                        let y = xs.get(5)?;

                        Some(x + y)
                    })
                    .unwrap_or_default(),
            })),
        ]
    })
}

pub fn parse_meminfo_output(output: &[u8]) -> Result<Vec<Record>, LustreCollectorError> {
    let output = std::str::from_utf8(output)?;

    let (mem_stats, state) = parse_meminfo()
        .easy_parse(output)
        .map_err(|err| err.map_position(|p| p.translate_position(output)))?;

    let params = crate::parser::params().join(" ");

    if !state.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Content left in input buffer. Please run and supply output to support: `lctl get_param {params}`"),
        )
        .into());
    }

    Ok(mem_stats)
}

fn parse_meminfo<'a, I>() -> impl Parser<I, Output = Vec<Record<'a>>>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    sep_end_by(
        choice((
            parse_meminfo_line().map(Some),
            (
                take_until::<String, _, _>(token(':')),
                token(':').skip(spaces()),
                digits().skip(till_newline()),
            )
                .map(|_| None),
        )),
        newline(),
    )
    .map(|xs: Vec<_>| xs.into_iter().flatten().collect())
}

fn parse_meminfo_line<'a, I>() -> impl Parser<I, Output = Record<'a>>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    choice((
        attempt(
            consume_line("MemTotal", "mem_total")
                .map(NodeStats::MemTotal)
                .map(Record::Node),
        ),
        attempt(
            consume_line("MemFree", "mem_free")
                .map(NodeStats::MemFree)
                .map(Record::Node),
        ),
        attempt(
            consume_line("SwapTotal", "swap_total")
                .map(NodeStats::SwapTotal)
                .map(Record::Node),
        ),
        attempt(
            consume_line("SwapFree", "swap_free")
                .map(NodeStats::SwapFree)
                .map(Record::Node),
        ),
    ))
}

fn consume_line<I>(name: &'static str, to: &'static str) -> impl Parser<I, Output = NodeStat<u64>>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        string_to(name, to),
        token(':').skip(spaces()),
        digits().skip(till_newline()),
    )
        .map(|(param, _, value)| NodeStat {
            param: Param(param),
            value,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use combine::parser::EasyParser;
    use insta::assert_debug_snapshot;

    const PROC_MEMINFO: &str = r#"MemTotal:        5943788 kB
MemFree:         4420248 kB
MemAvailable:    4707828 kB
Buffers:            5196 kB
Cached:           548160 kB
SwapCached:            0 kB
Active:           517648 kB
Inactive:         181844 kB
Active(anon):     190448 kB
Inactive(anon):    45888 kB
Active(file):     327200 kB
Inactive(file):   135956 kB
Unevictable:       84644 kB
Mlocked:           84644 kB
SwapTotal:       2097148 kB
SwapFree:        2097148 kB
Dirty:                28 kB
Writeback:             0 kB
AnonPages:        230740 kB
Mapped:           117352 kB
Shmem:             80044 kB
Slab:             141992 kB
SReclaimable:      71472 kB
SUnreclaim:        70520 kB
KernelStack:        7216 kB
PageTables:         9072 kB
NFS_Unstable:          0 kB
Bounce:                0 kB
WritebackTmp:          0 kB
CommitLimit:     5069040 kB
Committed_AS:     720820 kB
VmallocTotal:   34359738367 kB
VmallocUsed:      153492 kB
VmallocChunk:   34359496972 kB
HardwareCorrupted:     0 kB
AnonHugePages:     38912 kB
CmaTotal:              0 kB
CmaFree:               0 kB
HugePages_Total:       0
HugePages_Free:        0
HugePages_Rsvd:        0
HugePages_Surp:        0
Hugepagesize:       2048 kB
DirectMap4k:      245696 kB
DirectMap2M:     6045696 kB
"#;

    #[test]
    fn test_parse_meminfo_line() {
        let x = r#"MemTotal:        5943788 kB
"#;

        assert_debug_snapshot!(parse_meminfo_line().parse(x));
    }

    #[test]
    fn test_parse_meminfo() {
        assert_debug_snapshot!(parse_meminfo().easy_parse(PROC_MEMINFO));
    }

    #[test]
    fn test_empty_input() {
        assert_debug_snapshot!(parse_meminfo().easy_parse(""));
    }

    #[test]
    fn test_cpu_stats() {
        let x = "cpu  370338 12 481420 140010546 6313 0 39674 0 0 0";

        assert_debug_snapshot!(parse_cpustats().easy_parse(x));
    }
}

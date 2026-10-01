// Copyright (c) 2026 Google LLC
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

//! `osc.*.rpc_stats` and `mdc.*.rpc_stats` (`osc_rpc_stats_seq_show()`,
//! `mdc_rpc_stats_seq_show()`).
//!
//! ```text
//! read RPCs in flight:  0
//! pending read pages:   0
//!
//!             read            write
//! pages per rpc         rpcs   % cum % |       rpcs   % cum %
//! 1:                 0   0   0   |    2456543  48  48
//! ```
//!
//! Scalars and tables keep the kernel's names: the set differs by release and
//! build, and mdc adds a one-sided `modify` table. Newer mdc builds print `%%`
//! in the column header; tables stop once the cumulative percentage reaches 100.

use crate::{
    base_parsers::digits,
    brw_stats_parser::{bucket, count_pct_cum},
    key_value_parser::key_value,
    time::{StatsHeader, time_triple},
    types::{BrwStats, CountBucket, KeyValue, RpcStats},
};
use combine::{
    Parser, attempt, choice,
    error::ParseError,
    many, many1, optional,
    parser::{
        char::{newline, spaces, string},
        token::satisfy,
    },
    stream::Stream,
    token,
};

/// `2:              9773  44  50`
fn count_row<I>() -> impl Parser<I, Output = CountBucket>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    attempt(
        (digits().skip(token(':')), count_pct_cum().skip(newline()))
            .map(|(key, count)| CountBucket { key, count }),
    )
}

/// `pages per rpc         rpcs   % cum % |       rpcs   % cum %`: the name is
/// everything before the first run of two spaces, the unit the word after.
fn name_and_unit<I>() -> impl Parser<I, Output = (String, String)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    many1(satisfy(|c: char| c != '\n'))
        .skip(newline())
        .map(|line: String| {
            let (name, rest) = line.split_once("  ").unwrap_or((&line, ""));
            let unit = rest.split_whitespace().next().unwrap_or_default();
            (name.trim().to_string(), unit.to_string())
        })
}

/// A blank line, then `            read            write`, the name line, and the rows.
fn rw_histogram<I>() -> impl Parser<I, Output = BrwStats>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    attempt(
        (
            newline(),
            spaces().skip(string("read")),
            spaces().skip(string("write")).skip(newline()),
            name_and_unit(),
            many(attempt(bucket().skip(newline()))),
        )
            .map(|(_, _, _, (name, unit), buckets)| BrwStats {
                name,
                unit,
                buckets,
            }),
    )
}

/// A blank line, then `            modify`, the name line, and one-sided rows.
fn modify_histogram<I>() -> impl Parser<I, Output = Vec<CountBucket>>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    attempt(
        (
            newline(),
            spaces().skip(string("modify")).skip(newline()),
            name_and_unit(),
            many(count_row()),
        )
            .map(|(_, _, _, buckets)| buckets),
    )
}

enum Section {
    Scalar(KeyValue),
    Modify(Vec<CountBucket>),
    Histogram(BrwStats),
    /// The blank line before the scalars that follow the mdc `modify` histogram.
    Blank,
}

pub(crate) fn rpc_stats<I>() -> impl Parser<I, Output = (StatsHeader, RpcStats)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        optional(newline()).with(time_triple()),
        many(choice((
            key_value().map(Section::Scalar),
            modify_histogram().map(Section::Modify),
            rw_histogram().map(Section::Histogram),
            newline().map(|_| Section::Blank),
        ))),
    )
        .map(|(header, sections): (_, Vec<Section>)| {
            let mut stats = RpcStats {
                scalars: vec![],
                modify_rpcs_in_flight: None,
                histograms: vec![],
            };

            for section in sections {
                match section {
                    Section::Scalar(x) => stats.scalars.push(x),
                    Section::Modify(x) => stats.modify_rpcs_in_flight = Some(x),
                    Section::Histogram(x) => stats.histograms.push(x),
                    Section::Blank => {}
                }
            }

            (header, stats)
        })
        .message("while parsing rpc_stats")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::BrwStatsBucket;
    use combine::EasyParser;
    use insta::assert_debug_snapshot;

    const OSC: &str = "\nsnapshot_time:            1789952232.703630742 secs.nsecs\nstart_time:               1787864516.502993110 secs.nsecs\nelapsed_time:             2087716.200637632 secs.nsecs\nread RPCs in flight:  0\nwrite RPCs in flight: 0\nDIO RPCs in flight: 0\npending write pages:  0\npending read pages:   0\n\n\t\t\tread\t\t\twrite\npages per rpc         rpcs   % cum % |       rpcs   % cum %\n1:\t\t         0   0   0   |    52489  50  50\n2:\t\t         0   0   0   |    36540  35  85\n4:\t\t         1   0   0   |    14030  13  99\n8:\t\t       511  99 100   |      784   0 100\n\n\t\t\tread\t\t\twrite\nrpcs in flight        rpcs   % cum % |       rpcs   % cum %\n1:\t\t       512 100 100   |   103843 100 100\n\n\t\t\tread\t\t\twrite\noffset                rpcs   % cum % |       rpcs   % cum %\n0:\t\t       512 100 100   |   103843 100 100\n\n\t\t\tread\t\t\twrite\nRPC latency (us)       count   % cum % |       count   % cum %\n0:\t\t         0   0   0   |        0   0   0\n512:\t\t         3   0   0   |      567   0   0\n1024:\t\t       327  63  64   |    93264  89  90\n2048:\t\t       159  31  95   |     9820   9  99\n4096:\t\t        19   3  99   |      180   0  99\n8192:\t\t         3   0 100   |       10   0  99\n16384:\t\t         1   0 100   |        2   0 100\nosc.a361000-OST0001-osc-ffff949bc0626000.rpc_stats=\n";

    const MDC: &str = "\nsnapshot_time:            1789952232.668377739 secs.nsecs\nstart_time:               1787073662.340515449 secs.nsecs\nelapsed_time:             2878570.327862290 secs.nsecs\nmodify_RPCs_in_flight:  0\n\n\t\t\tmodify\nrpcs in flight        rpcs   % cum %\n0:\t\t         0   0   0\n1:\t\t      1383   6   6\n2:\t\t      9773  44  50\n3:\t\t      3486  15  66\n4:\t\t      7375  33 100\n\nread RPCs in flight:  0\nwrite RPCs in flight: 0\npending write pages:  0\npending read pages:   0\n\n\t\t\tread\t\t\twrite\npages per rpc         rpcs   % cum % |       rpcs   % cum %\n1:\t\t         0   0   0   |          0   0   0\n\n\t\t\tread\t\t\twrite\nrpcs in flight        rpcs   % cum % |       rpcs   % cum %\n1:\t\t         0   0   0   |          0   0   0\n\n\t\t\tread\t\t\twrite\noffset                rpcs   % cum % |       rpcs   % cum %\n0:\t\t         0   0   0   |          0   0   0\nmdc.a361000-MDT0000-mdc-ffff94d28d903800.rpc_stats=\n";

    #[test]
    fn parses_osc_with_dio_and_latency() {
        let ((header, stats), rest) = rpc_stats().easy_parse(OSC).unwrap();

        assert_eq!(
            rest,
            "osc.a361000-OST0001-osc-ffff949bc0626000.rpc_stats=\n"
        );
        assert_eq!(header.snapshot_time, "1789952232.703630742");
        assert_eq!(stats.scalars.len(), 5);
        assert_eq!(stats.scalars[2].name, "DIO RPCs in flight");
        assert!(stats.modify_rpcs_in_flight.is_none());
        assert_eq!(
            stats
                .histograms
                .iter()
                .map(|x| x.name.as_str())
                .collect::<Vec<_>>(),
            [
                "pages per rpc",
                "rpcs in flight",
                "offset",
                "RPC latency (us)"
            ]
        );
        assert_eq!(stats.histograms[3].buckets.len(), 7);
        assert_eq!(
            stats.histograms[3].buckets[2],
            BrwStatsBucket {
                name: 1024,
                read: 327,
                write: 93264
            }
        );

        assert_debug_snapshot!(stats);
    }

    #[test]
    fn parses_mdc_with_modify_histogram() {
        let ((_, stats), rest) = rpc_stats().easy_parse(MDC).unwrap();

        assert_eq!(
            rest,
            "mdc.a361000-MDT0000-mdc-ffff94d28d903800.rpc_stats=\n"
        );
        assert_eq!(stats.scalars[0].name, "modify_RPCs_in_flight");
        assert_eq!(stats.scalars.len(), 5);
        assert_eq!(
            stats.modify_rpcs_in_flight.as_ref().unwrap()[2],
            CountBucket {
                key: 2,
                count: 9773
            }
        );
        assert_eq!(stats.histograms.len(), 3);

        assert_debug_snapshot!(stats);
    }

    #[test]
    fn literal_double_percent_headers_are_accepted() {
        let input = MDC.replace("% cum %", "%% cum %%");

        let ((_, stats), _) = rpc_stats().easy_parse(input.as_str()).unwrap();

        assert_eq!(stats.histograms[0].name, "pages per rpc");
        assert_eq!(stats.modify_rpcs_in_flight.unwrap().len(), 5);
    }

    #[test]
    fn header_only_when_idle() {
        let input = "\nsnapshot_time:            1789952232.703585899 secs.nsecs\nread RPCs in flight:  0\nosc.x.rpc_stats=\n";

        let ((_, stats), rest) = rpc_stats().easy_parse(input).unwrap();

        assert_eq!(stats.scalars.len(), 1);
        assert!(stats.histograms.is_empty());
        assert_eq!(rest, "osc.x.rpc_stats=\n");
    }
}

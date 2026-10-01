// Copyright (c) 2026 Google LLC
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

//! `osc.*.osc_stats` and `mdc.*.mdc_stats` (`osc_stats_seq_show()`). 2.12 to
//! 2.14.52 print a third line, `lockless_truncate` (LU-14838).

use crate::{
    base_parsers::digits,
    time::{StatsHeader, time_triple},
    types::LocklessStats,
};
use combine::{
    Parser, attempt,
    error::ParseError,
    optional,
    parser::char::{newline, spaces, string},
    stream::Stream,
};

fn counter<I>(name: &'static str) -> impl Parser<I, Output = u64>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    string(name).skip(spaces()).with(digits()).skip(newline())
}

pub(crate) fn lockless_stats<I>() -> impl Parser<I, Output = (StatsHeader, LocklessStats)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        optional(newline()).with(time_triple()),
        counter("lockless_write_bytes"),
        counter("lockless_read_bytes"),
        optional(attempt(counter("lockless_truncate"))),
    )
        .map(|(header, write_bytes, read_bytes, truncates)| {
            (
                header,
                LocklessStats {
                    write_bytes,
                    read_bytes,
                    truncates,
                },
            )
        })
        .message("while parsing lockless stats")
}

#[cfg(test)]
mod tests {
    use super::*;
    use combine::EasyParser;

    #[test]
    fn parses_real_output() {
        let input = "\nsnapshot_time:            1789952232.698225730 secs.nsecs\nstart_time:               1787073662.351805909 secs.nsecs\nelapsed_time:             2878570.346419821 secs.nsecs\nlockless_write_bytes\t\t4096\nlockless_read_bytes\t\t0\nosc.fs-OST0000-osc-ffff94d28d903800.osc_stats=\n";

        let ((header, stats), rest) = lockless_stats().easy_parse(input).unwrap();

        assert_eq!(header.snapshot_time, "1789952232.698225730");
        assert_eq!(header.start_time.as_deref(), Some("1787073662.351805909"));
        assert_eq!(
            stats,
            LocklessStats {
                write_bytes: 4096,
                read_bytes: 0,
                truncates: None,
            }
        );
        assert_eq!(rest, "osc.fs-OST0000-osc-ffff94d28d903800.osc_stats=\n");
    }

    #[test]
    fn parses_pre_2_14_53_output() {
        let input = "\nsnapshot_time:            1689697369.331040915 secs.nsecs\nlockless_write_bytes\t\t0\nlockless_read_bytes\t\t8192\nlockless_truncate\t\t3\nosc.fs-OST0001-osc-ffff94d28d903800.osc_stats=\n";

        let ((header, stats), rest) = lockless_stats().easy_parse(input).unwrap();

        assert_eq!(header.start_time, None);
        assert_eq!(
            stats,
            LocklessStats {
                write_bytes: 0,
                read_bytes: 8192,
                truncates: Some(3),
            }
        );
        assert_eq!(rest, "osc.fs-OST0001-osc-ffff94d28d903800.osc_stats=\n");
    }
}

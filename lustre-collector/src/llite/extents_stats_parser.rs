// Copyright (c) 2026 Google LLC
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

//! `llite.*.extents_stats` and `llite.*.extents_stats_per_process`
//! (`ll_rw_extents_stats_seq_show()`, `ll_rw_extents_stats_pp_seq_show()` and
//! `ll_display_extents_info()` in `lustre/llite/lproc_llite.c`).
//!
//! ```text
//!                                read       |                write
//!       extents            calls    % cum%  |          calls    % cum%
//!    0K -    4K :           9833    9    9  |           9065   30   30
//!   32K -   64K :          83121   79  100  |           1230    4   63
//! ```
//!
//! The character after the upper bound is `+` on the overflow bucket. Rows
//! stop once both cumulative percentages reach 100.

use super::rw_stats;
use crate::{
    base_parsers::digits,
    brw_stats_parser::{count_pct_cum, human_to_bytes},
    types::{ExtentsBucket, ProcessExtents, RwStats},
};
use combine::{
    Parser, attempt, choice,
    error::ParseError,
    many, one_of, optional,
    parser::char::{newline, spaces, string},
    stream::Stream,
    token,
};

/// `   0K -    4K :           9833    9    9  |           9065   30   30`
fn row<I>() -> impl Parser<I, Output = ExtentsBucket>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    attempt(
        (
            spaces().with((digits(), optional(one_of("KkMmGg".chars()))).map(human_to_bytes)),
            spaces()
                .skip(token('-'))
                .skip(spaces())
                .with((digits(), optional(one_of("KkMmGg".chars()))).map(human_to_bytes)),
            choice((token('+').map(|_| true), token(' ').map(|_| false))).skip(token(':')),
            count_pct_cum(),
            spaces().skip(token('|')),
            count_pct_cum().skip(newline()),
        )
            .map(
                |(lower_bytes, upper_bytes, overflow, read_calls, _, write_calls)| ExtentsBucket {
                    lower_bytes,
                    upper_bytes,
                    overflow,
                    read_calls,
                    write_calls,
                },
            ),
    )
}

fn column_headers<I>() -> impl Parser<I, Output = ()>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        spaces().skip(string("read")),
        spaces().skip(token('|')),
        spaces().skip(string("write")).skip(newline()),
        spaces().skip(string("extents")),
        spaces().skip(string("calls")),
        spaces().skip(token('%')),
        spaces().skip(string("cum%")),
        spaces().skip(token('|')),
        spaces().skip(string("calls")),
        spaces().skip(token('%')),
        spaces().skip(string("cum%")).skip(newline()),
    )
        .map(|_| ())
}

pub(crate) fn extents_stats<I>() -> impl Parser<I, Output = RwStats<Vec<ExtentsBucket>>>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    rw_stats((column_headers(), many(row())).map(|(_, rows)| rows))
        .message("while parsing extents_stats")
}

fn process<I>() -> impl Parser<I, Output = ProcessExtents>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    attempt(
        (
            newline()
                .skip(string("PID:"))
                .skip(spaces())
                .with(digits())
                .skip(newline()),
            many(row()),
        )
            .map(|(pid, buckets)| ProcessExtents { pid, buckets }),
    )
}

pub(crate) fn extents_stats_per_process<I>() -> impl Parser<I, Output = RwStats<Vec<ProcessExtents>>>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    rw_stats((column_headers(), many(process())).map(|(_, xs)| xs))
        .message("while parsing extents_stats_per_process")
}

#[cfg(test)]
mod tests {
    use super::*;
    use combine::EasyParser;
    use insta::assert_debug_snapshot;

    const BUSY: &str = "\nsnapshot_time:            1789953552.214577612 secs.nsecs\nstart_time:               1789953080.395147468 secs.nsecs\nelapsed_time:             471.819430144 secs.nsecs\n                               read       |                write\n      extents            calls    % cum%  |          calls    % cum%\n   0K -    4K :           9833    9    9  |           9065   30   30\n   4K -    8K :           4885    4   14  |           4142   13   44\n   8K -   16K :           3693    3   17  |           2894    9   53\n  16K -   32K :           2994    2   20  |           1689    5   59\n  32K -   64K :          83121   79  100  |           1230    4   63\n  64K -  128K :              0    0  100  |            720    2   65\n 128K -  256K :              0    0  100  |            944    3   68\n 256K -  512K :              0    0  100  |           9329   31  100\nllite.a361000-ffff949bc0626000.extents_stats_per_process=\n";

    const IDLE: &str = "\nsnapshot_time:            1789953552.214544163 secs.nsecs\nstart_time:               1789953080.395126028 secs.nsecs\nelapsed_time:             471.819418135 secs.nsecs\n                               read       |                write\n      extents            calls    % cum%  |          calls    % cum%\n   0K -    4K :              0    0    0  |              0    0    0\nllite.x.extents_stats=\n";

    const DISABLED: &str = "\ndisabled\n write anything to this file to activate, then '0' or 'disable' to deactivate\nllite.x.extents_stats=\n";

    #[test]
    fn parses_busy_mount() {
        let (xs, rest) = extents_stats().easy_parse(BUSY).unwrap();

        assert_eq!(
            rest,
            "llite.a361000-ffff949bc0626000.extents_stats_per_process=\n"
        );

        let RwStats::Enabled { header, value } = &xs else {
            panic!("expected enabled");
        };
        assert_eq!(header.snapshot_time, "1789953552.214577612");
        assert_eq!(value.len(), 8);
        assert_eq!(
            value[4],
            ExtentsBucket {
                lower_bytes: 32 << 10,
                upper_bytes: 64 << 10,
                overflow: false,
                read_calls: 83121,
                write_calls: 1230
            }
        );

        assert_debug_snapshot!(xs);
    }

    #[test]
    fn idle_mount_has_one_zero_row() {
        let (xs, rest) = extents_stats().easy_parse(IDLE).unwrap();

        let RwStats::Enabled { value, .. } = xs else {
            panic!("expected enabled");
        };
        assert_eq!(value.len(), 1);
        assert_eq!(value[0].read_calls, 0);
        assert_eq!(rest, "llite.x.extents_stats=\n");
    }

    #[test]
    fn disabled() {
        let (xs, rest) = extents_stats().easy_parse(DISABLED).unwrap();

        assert_eq!(xs, RwStats::Disabled);
        assert_eq!(rest, "llite.x.extents_stats=\n");
    }

    /// Not seen in a capture; built from the kernel's format string.
    #[test]
    fn overflow_bucket() {
        let input = "   8G -   16G+:              7  100  100  |              0    0  100\n";

        let (x, rest) = row().easy_parse(input).unwrap();

        assert!(x.overflow);
        assert_eq!(x.lower_bytes, 8 << 30);
        assert_eq!(x.upper_bytes, 16 << 30);
        assert_eq!(x.read_calls, 7);
        assert_eq!(rest, "");
    }

    #[test]
    fn per_process() {
        let input = "\nsnapshot_time:            1789953552.218611921 secs.nsecs\nstart_time:               1789953080.395147468 secs.nsecs\nelapsed_time:             471.823464453 secs.nsecs\n                               read       |                write\n      extents            calls    % cum%  |          calls    % cum%\n\nPID: 1748326\n   0K -    4K :            443   26   26  |              0    0    0\n   4K -    8K :            424   25   51  |              0    0    0\n\nPID: 1748455\n   0K -    4K :            444    3    3  |              0    0    0\nllite.x.offset_stats=\n";

        let (xs, rest) = extents_stats_per_process().easy_parse(input).unwrap();

        assert_eq!(rest, "llite.x.offset_stats=\n");

        let RwStats::Enabled { value, .. } = &xs else {
            panic!("expected enabled");
        };
        assert_eq!(value.len(), 2);
        assert_eq!(value[0].pid, 1748326);
        assert_eq!(value[0].buckets.len(), 2);
        assert_eq!(value[1].pid, 1748455);
        assert_eq!(value[1].buckets[0].read_calls, 444);

        assert_debug_snapshot!(xs);
    }

    #[test]
    fn per_process_idle_has_no_blocks() {
        let input = "\nsnapshot_time:            1789953552.218533061 secs.nsecs\nstart_time:               1789953080.395126028 secs.nsecs\nelapsed_time:             471.823407033 secs.nsecs\n                               read       |                write\n      extents            calls    % cum%  |          calls    % cum%\nllite.x.extents_stats_per_process=\n";

        let (xs, rest) = extents_stats_per_process().easy_parse(input).unwrap();

        let RwStats::Enabled { value, .. } = xs else {
            panic!("expected enabled");
        };
        assert!(value.is_empty());
        assert_eq!(rest, "llite.x.extents_stats_per_process=\n");
    }
}

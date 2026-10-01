// Copyright (c) 2026 Google LLC
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

//! `llite.*.read_ahead_stats`. Before 2.14.52 (LU-13705) the names contain
//! spaces and hyphens (`read-ahead to EOF         1 samples [pages]`), which
//! the generic stats parser cannot read; they are normalized to the 2.14.52
//! spelling so the label is the same on every release.

use crate::{
    base_parsers::till_newline, stats_parser::stats_header_and, time::StatsHeader, types::Stat,
};
use combine::{
    Parser, attempt,
    error::{ParseError, StreamError},
    many,
    parser::char::newline,
    stream::{Stream, StreamErrorFor},
};

/// `ra_stat_string[]` at v2_14_0 mapped to the 2.14.52+ names.
fn normalize(name: &str) -> String {
    match name {
        "read-ahead to EOF" => "readahead_to_eof".to_string(),
        "hit max r-a issue" => "hit_max_readahead_issue".to_string(),
        _ => name.replace(' ', "_"),
    }
}

/// `name<spaces><samples> samples [<units>]<optional min max sum sumsq>`.
fn parse_line(line: &str) -> Option<Stat> {
    let (left, right) = line.split_once(" samples [")?;
    let (name, samples) = left.trim_end().rsplit_once(char::is_whitespace)?;
    let samples = samples.parse().ok()?;
    let (units, rest) = right.split_once(']')?;
    let nums: Vec<u64> = rest
        .split_whitespace()
        .map(str::parse)
        .collect::<Result<_, _>>()
        .ok()?;
    let (min, max, sum, sumsquare) = match nums.as_slice() {
        [] => (None, None, None, None),
        [min, max, sum] => (Some(*min), Some(*max), Some(*sum), None),
        [min, max, sum, sq] => (Some(*min), Some(*max), Some(*sum), Some(*sq)),
        _ => return None,
    };

    Some(Stat {
        name: normalize(name.trim()),
        units: units.to_string(),
        samples,
        min,
        max,
        sum,
        sumsquare,
    })
}

fn stat<I>() -> impl Parser<I, Output = Stat>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    // `attempt`: the next parameter header must stay unconsumed.
    attempt(till_newline().skip(newline()).and_then(|line: String| {
        parse_line(&line)
            .ok_or_else(|| StreamErrorFor::<I>::unexpected_static_message("read_ahead_stats line"))
    }))
}

pub(crate) fn read_ahead_stats<I>() -> impl Parser<I, Output = (StatsHeader, Vec<Stat>)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    stats_header_and(many(stat())).message("while parsing read_ahead_stats")
}

#[cfg(test)]
mod tests {
    use super::*;
    use combine::EasyParser;

    #[test]
    fn parses_pre_2_14_52_names() {
        let input = "\nsnapshot_time             1689697369.331040915 secs.nsecs\nhits                      12 samples [pages]\nmisses                    4 samples [pages]\nreadpage not consecutive  3 samples [pages]\nmiss inside window        1 samples [pages]\nfailed grab_cache_page    1 samples [pages]\nread-ahead to EOF         7 samples [pages]\nhit max r-a issue         2 samples [pages]\nwrong page from grab_cache_page 1 samples [pages]\nfailed to fast read       5 samples [pages]\nllite.x.extents_stats=\n";

        let ((header, stats), rest) = read_ahead_stats().easy_parse(input).unwrap();

        assert_eq!(rest, "llite.x.extents_stats=\n");
        assert_eq!(header.snapshot_time, "1689697369.331040915");
        let names: Vec<(&str, u64)> = stats.iter().map(|s| (s.name.as_str(), s.samples)).collect();
        assert_eq!(
            names,
            vec![
                ("hits", 12),
                ("misses", 4),
                ("readpage_not_consecutive", 3),
                ("miss_inside_window", 1),
                ("failed_grab_cache_page", 1),
                ("readahead_to_eof", 7),
                ("hit_max_readahead_issue", 2),
                ("wrong_page_from_grab_cache_page", 1),
                ("failed_to_fast_read", 5),
            ]
        );
        assert!(stats.iter().all(|s| s.units == "pages" && s.min.is_none()));
    }

    #[test]
    fn parses_current_names() {
        let input = "\nsnapshot_time             1789952232.698225730 secs.nsecs\nstart_time                1787073662.351805909 secs.nsecs\nelapsed_time              2878570.346419821 secs.nsecs\nhits                      100 samples [pages] 1 1 100 100\nreadahead_to_eof          3 samples [pages]\nosc.x.stats=\n";

        let ((header, stats), rest) = read_ahead_stats().easy_parse(input).unwrap();

        assert_eq!(rest, "osc.x.stats=\n");
        assert_eq!(header.start_time.as_deref(), Some("1787073662.351805909"));
        assert_eq!(stats.len(), 2);
        assert_eq!(stats[0].sumsquare, Some(100));
        assert_eq!(stats[1].name, "readahead_to_eof");
    }

    #[test]
    fn idle_block_has_header_only() {
        let input =
            "\nsnapshot_time             1789952232.698225730 secs.nsecs\nllite.x.extents_stats=\n";

        let ((_, stats), rest) = read_ahead_stats().easy_parse(input).unwrap();

        assert!(stats.is_empty());
        assert_eq!(rest, "llite.x.extents_stats=\n");
    }
}

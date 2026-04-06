// Copyright (c) 2026 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use std::collections::BTreeMap;

use crate::{
    base_parsers::digits,
    time::time_triple,
    types::{BrwStats, BrwStatsBucket},
};
use combine::{
    Parser, attempt, between,
    error::ParseError,
    many, optional,
    parser::char::{newline, spaces, string},
    sep_end_by,
    stream::Stream,
    token,
};

/// Parses a latency bucket entry like `512us: 3`
fn latency_bucket<I>() -> impl Parser<I, Output = (u64, u64)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        digits().skip(string("us")),
        token(':').skip(spaces()),
        digits(),
    )
        .map(|(latency, _, count)| (latency, count))
}

/// Parses an opsize like `4K` or `1024K`
fn opsize<I>() -> impl Parser<I, Output = String>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (digits(), token('K')).map(|(n, _)| format!("{n}K"))
}

/// Parses a single line like `rd_1024K: { 512us: 3, 1024us: 1, 4096us: 5, }`
fn latency_line<I>() -> impl Parser<I, Output = (String, String, Vec<(u64, u64)>)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    let operation = attempt(string("rd_").map(|_| "read".to_string()))
        .or(string("wr_").map(|_| "write".to_string()));

    (
        operation,
        opsize().skip(token(':').skip(spaces())),
        between(
            token('{').skip(spaces()),
            token('}'),
            sep_end_by(latency_bucket(), token(',').skip(spaces())),
        )
        .skip(optional(newline())),
    )
}

/// Parses the full io_latency_stats output into Vec<BrwStats>.
/// Groups rd/wr lines by opsize into BrwStats entries with name "io_time_{opsize}".
pub(crate) fn io_latency_stats<I>() -> impl Parser<I, Output = Vec<BrwStats>>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        newline(),
        string("io_latency_by_size:").skip(newline()),
        time_triple(),
        many::<Vec<_>, _, _>(latency_line()),
    )
        .map(|(_, _, _, lines)| {
            let mut map: BTreeMap<String, BTreeMap<u64, (u64, u64)>> = BTreeMap::new();

            for (operation, size, buckets) in lines {
                let entry = map.entry(size).or_default();

                for (latency, count) in buckets {
                    let bucket = entry.entry(latency).or_insert((0, 0));

                    if operation == "read" {
                        bucket.0 += count;
                    } else {
                        bucket.1 += count;
                    }
                }
            }

            map.into_iter()
                .map(|(size, buckets)| BrwStats {
                    name: format!("io_time_{size}"),
                    unit: "ios".to_string(),
                    buckets: buckets
                        .into_iter()
                        .map(|(latency, (read, write))| BrwStatsBucket {
                            name: latency,
                            read,
                            write,
                        })
                        .collect(),
                })
                .collect()
        })
}

#[cfg(test)]
mod tests {
    use combine::EasyParser;
    use insta::assert_debug_snapshot;

    use super::*;

    #[test]
    fn test_latency_bucket() {
        let result = latency_bucket().easy_parse("512us: 3").unwrap();

        assert_eq!(result.0, (512, 3));
    }

    #[test]
    fn test_latency_line_read() {
        let result = latency_line()
            .easy_parse("rd_1024K: { 512us: 3, 1024us: 1, 4096us: 5, }\n")
            .unwrap();

        assert_eq!(result.0.0, "read");
        assert_eq!(result.0.1, "1024K");
        assert_eq!(result.0.2, vec![(512, 3), (1024, 1), (4096, 5)]);
    }

    #[test]
    fn test_latency_line_write() {
        let result = latency_line()
            .easy_parse("wr_1024K: { 1024us: 7, 4096us: 1, 8192us: 1, }\n")
            .unwrap();

        assert_eq!(result.0.0, "write");
        assert_eq!(result.0.1, "1024K");
        assert_eq!(result.0.2, vec![(1024, 7), (4096, 1), (8192, 1)]);
    }

    #[test]
    fn test_io_latency_stats() {
        let input = r#"
io_latency_by_size:
snapshot_time:  1775128065.882649010
start_time:     1775125270.244958271
elapsed_time:   2795.637690739
rd_1024K: { 512us: 3, 1024us: 1, 4096us: 5, }
wr_1024K: { 1024us: 7, 4096us: 1, 8192us: 1, }
"#;

        let result = io_latency_stats().easy_parse(input).unwrap();

        assert_debug_snapshot!(result);
    }

    #[test]
    fn test_io_latency_stats_multiple_sizes() {
        let input = r#"
io_latency_by_size:
snapshot_time:  1775128281.261562279
start_time:     1775125270.250717218
elapsed_time:   3011.010845061
rd_4K: { 256us: 933, 512us: 83, 1024us: 1, 2048us: 5, }
wr_4K: { 256us: 934, 512us: 86, 1024us: 1, 16384us: 1, 65536us: 1, }
rd_1024K: { 512us: 3, 1024us: 1, }
wr_1024K: { 1024us: 7, }
"#;

        let result = io_latency_stats().easy_parse(input).unwrap();

        assert_debug_snapshot!(result);
    }

    #[test]
    fn test_io_latency_stats_empty() {
        let input = r#"
io_latency_by_size:
snapshot_time:  1775128281.261349828
start_time:     1775125270.194383175
elapsed_time:   3011.066966653
"#;

        let result = io_latency_stats().easy_parse(input).unwrap();

        assert_eq!(result.0, vec![]);
    }
}

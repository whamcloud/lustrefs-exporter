// Copyright (c) 2026 Google LLC
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

//! `name: value` blocks (`statahead_stats`, `unstable_stats`, `stats_compr`).
//! The key set varies by release and build, so callers pick the names they know.

use crate::{base_parsers::digits, types::KeyValue};
use combine::{
    Parser, attempt,
    error::ParseError,
    many1,
    parser::{
        char::{newline, spaces},
        token::satisfy,
    },
    stream::Stream,
    token,
};

pub(crate) fn key_value<I>() -> impl Parser<I, Output = KeyValue>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    // `attempt`: the next parameter header has no `:` and must stay unconsumed.
    attempt(
        (
            many1(satisfy(|c: char| c != ':' && c != '\n' && c != '=')).skip(token(':')),
            spaces().with(digits()).skip(newline()),
        )
            .map(|(name, value): (String, u64)| KeyValue { name, value }),
    )
}

pub(crate) fn key_values<I>() -> impl Parser<I, Output = Vec<KeyValue>>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    newline()
        .with(many1(key_value()))
        .message("while parsing name: value lines")
}

#[cfg(test)]
mod tests {
    use super::*;
    use combine::EasyParser;

    #[test]
    fn parses_padded_lines_and_stops_at_next_param() {
        let input = "\nunstable_check:            1\nunstable_pages:            0\nunstable_mb:               0\nllite.fs-ffff94d28d903800.unstable_stats=\n";

        let (xs, rest) = key_values().easy_parse(input).unwrap();

        assert_eq!(
            xs,
            vec![
                KeyValue {
                    name: "unstable_check".to_string(),
                    value: 1
                },
                KeyValue {
                    name: "unstable_pages".to_string(),
                    value: 0
                },
                KeyValue {
                    name: "unstable_mb".to_string(),
                    value: 0
                },
            ]
        );
        assert_eq!(rest, "llite.fs-ffff94d28d903800.unstable_stats=\n");
    }

    #[test]
    fn names_may_contain_spaces() {
        let input = "\nstatahead total: 170075\nstatahead wrong: 380\nagl total: 170075\n";

        let (xs, rest) = key_values().easy_parse(input).unwrap();

        assert_eq!(xs[0].name, "statahead total");
        assert_eq!(xs[0].value, 170075);
        assert_eq!(xs.len(), 3);
        assert_eq!(rest, "");
    }

    #[test]
    fn rejects_non_numeric_value() {
        assert!(key_values().easy_parse("\nfoo: bar\n").is_err());
    }
}

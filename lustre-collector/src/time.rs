// Copyright (c) 2021 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::base_parsers::{digits, till_newline};
use combine::stream::Stream;
use combine::{Parser, optional, token};
use combine::{
    attempt,
    parser::char::{spaces, string},
};
use combine::{error::ParseError, parser::char::newline};

fn time<I>(name: &'static str) -> impl Parser<I, Output = String>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        string(name).skip(optional(token(':'))),
        spaces(),
        digits().skip(token('.')),
        digits().skip(till_newline()),
    )
        .map(|(_, _, secs, nsecs)| format!("{secs}.{nsecs}"))
}

pub(crate) fn time_triple<I>() -> impl Parser<I, Output = String>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        time("snapshot_time")
            .message("While parsing snapshot_time")
            .skip(newline()),
        optional(
            attempt(
                time("start_time")
                    .skip(newline())
                    .message("While parsing start_time"),
            )
            .and(
                time("elapsed_time")
                    .skip(newline())
                    .message("While parsing elapsed_time"),
            ),
        ),
    )
        .map(|(time, _)| time)
}

#[cfg(test)]
mod tests {
    use combine::EasyParser;
    use insta::assert_debug_snapshot;

    use super::*;

    #[test]
    fn test_time() {
        let x = r#"snapshot_time:         1534158712.738772898 (secs.nsecs)
"#;

        let result = time("snapshot_time").parse(x);

        assert_eq!(result, Ok(("1534158712.738772898".to_string(), "\n",)));
    }
    #[test]
    fn test_time_no_colon() {
        let x = r#"snapshot_time             1534769431.137892896 secs.nsecs
"#;

        let result = time("snapshot_time").parse(x);

        assert_eq!(result, Ok(("1534769431.137892896".to_string(), "\n")));
    }

    #[test]
    fn test_time_triple() {
        let x = r#"snapshot_time             1684948453.142852820 secs.nsecs
start_time                1684946875.504329012 secs.nsecs
elapsed_time              1577.638523808 secs.nsecs
"#;

        let result = time_triple().easy_parse(x).unwrap();

        assert_debug_snapshot!(result);
    }

    #[test]
    fn test_time_triple_back_compat() {
        let x = r#"snapshot_time             1596728874.484750908 secs.nsecs
req_waittime              31280 samples [usec] 11 2695 5020274 1032267156

"#;

        let result = time_triple().easy_parse(x).unwrap();

        assert_debug_snapshot!(result);
    }
}

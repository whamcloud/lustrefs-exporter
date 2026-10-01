// Copyright (c) 2021 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::base_parsers::{digits, till_newline};
use combine::stream::Stream;
use combine::{Parser, many1, optional, skip_many, token};
use combine::{
    attempt,
    parser::char::{digit, spaces, string},
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
        // Fixed-width field: leading zeros matter, and mdc rpc_stats before
        // 2.14.56 padded it with spaces ("%9lu").
        skip_many(token(' '))
            .with(many1::<String, _, _>(digit()))
            .skip(till_newline()),
    )
        .map(|(_, _, secs, nsecs)| format!("{secs}.{nsecs:0>9}"))
}
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct StatsHeader {
    pub snapshot_time: String,
    pub start_time: Option<String>,
}

pub(crate) fn time_triple<I>() -> impl Parser<I, Output = StatsHeader>
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
        .map(|(snapshot, optional_start)| StatsHeader {
            snapshot_time: snapshot,
            start_time: optional_start.map(|(start, _elapsed)| start),
        })
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

    /// 1566017453.009677077 is not 1566017453.9677077.
    #[test]
    fn test_time_leading_zero_nsecs() {
        let x = "snapshot_time             1566017453.009677077 secs.nsecs\n";

        let result = time("snapshot_time").parse(x);

        assert_eq!(result, Ok(("1566017453.009677077".to_string(), "\n")));
    }

    #[test]
    fn test_time_triple_leading_zero_start_time() {
        let x = "snapshot_time             1684948453.142852820 secs.nsecs\nstart_time                1684946875.004329012 secs.nsecs\nelapsed_time              1577.038523808 secs.nsecs\n";

        let (result, _) = time_triple().easy_parse(x).unwrap();

        assert_eq!(result.start_time.as_deref(), Some("1684946875.004329012"));
    }

    /// mdc rpc_stats before 2.14.56: "%9lu", space padded.
    #[test]
    fn test_time_space_padded_nanoseconds() {
        let x = "snapshot_time:         1534158712.  4567890 (secs.nsecs)\n";

        let result = time("snapshot_time").parse(x);

        assert_eq!(result, Ok(("1534158712.004567890".to_string(), "\n")));
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

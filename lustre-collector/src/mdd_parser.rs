// Copyright (c) 2024 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    base_parsers::{digits, param, period, target, till_newline, till_period},
    types::{Param, Record, Target, TargetStat, TargetStats, TargetVariant},
    ChangeLogUser, ChangelogStat,
};
use combine::{
    attempt, choice,
    error::{ParseError, StreamError},
    many,
    parser::char::{newline, spaces, string},
    stream::{Stream, StreamErrorFor},
    token, Parser,
};

pub(crate) const MDD: &str = "mdd";
pub(crate) const CHANGELOG_USERS: &str = "changelog_users";
pub(crate) fn params() -> Vec<String> {
    vec![format!("{MDD}.*.{CHANGELOG_USERS}")]
}

#[derive(Debug)]
enum MddStat {
    /// Changelog stat
    ChangeLog(ChangelogStat),
}

fn target_and_variant<I>() -> impl Parser<I, Output = (Target, TargetVariant)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        attempt(string("mdd").skip(till_period())).skip(period()),
        target().skip(period()),
    )
        .and_then(move |(_, x)| -> Result<_, _> {
            let variant = match (&x).try_into() {
                Ok(x) => x,
                Err(e) => return Err(StreamErrorFor::<I>::other(e)),
            };

            Ok((x, variant))
        })
        .message("while parsing target_and_variant")
}

fn table_headers<I>() -> impl Parser<I, Output = ()>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (string("ID"), till_newline()).map(|_| ())
}

fn table_rows<I>() -> impl Parser<I, Output = Vec<ChangeLogUser>>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    many(attempt((
        target(),
        spaces(),
        digits(),
        spaces(),
        token('('),
        digits(),
        token(')'),
        till_newline().skip(newline()),
    )))
    .map(|x: Vec<_>| {
        x.iter()
            .map(|x| ChangeLogUser {
                user: x.0.to_string(),
                index: x.2,
                idle_secs: x.5,
            })
            .collect()
    })
}

fn mdd_stat<I>() -> impl Parser<I, Output = (Param, MddStat)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    choice(((
        param(CHANGELOG_USERS),
        (
            newline(),
            string("current_index: "),
            digits(),
            newline(),
            table_headers(),
            newline(),
            table_rows(),
        )
            .map(|(_, _, x, _, _, _, y)| {
                MddStat::ChangeLog(ChangelogStat {
                    current_index: x,
                    users: y,
                })
            }),
    )
        .message("while parsing changelog"),))
}

pub(crate) fn parse<I>() -> impl Parser<I, Output = Record>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (target_and_variant(), mdd_stat())
        .map(|((target, kind), (param, stat))| match stat {
            MddStat::ChangeLog(value) => TargetStats::Changelog(TargetStat {
                kind,
                target,
                param,
                value,
            }),
        })
        .map(Record::Target)
        .message("while parsing mdd")
}

#[cfg(test)]
mod tests {
    use combine::{many, EasyParser};
    use insta::assert_debug_snapshot;

    use super::*;

    #[test]
    fn test_mdd_stats() {
        static FIXTURE: &str = include_str!("fixtures/mdd.txt");

        let result = many::<Vec<_>, _, _>(parse())
            .easy_parse(FIXTURE)
            .map_err(|err| err.map_position(|p| p.translate_position(FIXTURE)))
            .unwrap();

        assert_debug_snapshot!(result);
    }
}

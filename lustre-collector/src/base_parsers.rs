// Copyright (c) 2021 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use combine::{
    Parser, attempt,
    error::{Format, ParseError},
    many1, one_of,
    parser::{
        char::{alpha_num, digit, newline, string},
        repeat::take_until,
    },
    stream::Stream,
    token, unexpected, unexpected_any, value,
};

use crate::types::{Param, Target};

pub mod w {
    use crate::types::{Param, Target};
    use winnow::{
        ModalResult,
        combinator::terminated,
        prelude::*,
        stream::AsChar,
        token::{literal, take_while},
    };

    /// Parses a target name
    pub fn target(input: &mut &str) -> ModalResult<Target> {
        take_while(1.., |c: char| {
            AsChar::is_alphanum(c) || c == '_' || c == '-'
        })
        .map(|s: &str| Target(s.into()))
        .parse_next(input)
    }

    pub fn param(param_name: &str) -> impl FnMut(&mut &str) -> ModalResult<Param> {
        let param_name = param_name.to_string();
        move |input: &mut &str| {
            terminated(literal(param_name.as_str()), "=")
                .map(|s: &str| Param(s.into()))
                .parse_next(input)
        }
    }
}

pub(crate) fn period<I>() -> impl Parser<I, Output = char>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    token('.')
}

pub(crate) fn equals<I>() -> impl Parser<I, Output = char>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    token('=')
}

pub(crate) fn word<I>() -> impl Parser<I, Output = String>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    many1(alpha_num().or(token('_')))
}

/// Parses a target name
pub(crate) fn target<I>() -> impl Parser<I, Output = Target>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    many1(alpha_num().or(one_of("_-".chars()))).map(Target)
}

/// Takes many consecutive digits and
/// returns them as u64
pub(crate) fn digits<I>() -> impl Parser<I, Output = u64>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    many1(digit()).then(|x: String| match x.parse::<u64>() {
        Ok(n) => value(n).left(),
        Err(e) => unexpected_any(Format(e)).right(),
    })
}

pub(crate) fn till_newline<I>() -> impl Parser<I, Output = String>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    take_until(newline())
}

pub(crate) fn till_period<I>() -> impl Parser<I, Output = String>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    take_until(period())
}

pub(crate) fn string_to<I>(x: &'static str, y: &'static str) -> impl Parser<I, Output = String>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    string(x).map(move |_| String::from(y))
}

pub(crate) fn not_words<I>(xs: &'static [&'static str]) -> impl Parser<I, Output = String>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    attempt(word().then(move |y| {
        for &x in xs {
            if x == y {
                return unexpected(x).map(|_| "".to_string()).right();
            }
        }

        value(y).left()
    }))
}

pub(crate) fn param<I>(x: &'static str) -> impl Parser<I, Output = Param>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    attempt(string(x).skip(equals()))
        .map(|x| Param(x.to_string()))
        .message("while getting param")
}

pub(crate) fn param_period<I>(x: &'static str) -> impl Parser<I, Output = Param>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    attempt(string(x).skip(token('.')))
        .map(|x| Param(x.to_string()))
        .message("while getting param")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Param;

    #[test]
    fn test_param() {
        let result = param("memused").parse("memused=77991501\n");

        assert_eq!(result, Ok((Param("memused".to_string()), "77991501\n")))
    }

    #[test]
    fn test_param_period() {
        let result = param_period("exports").parse("exports.1.2.3.130@o2ib.stats=Y\n");

        assert_eq!(
            result,
            Ok((Param("exports".to_string()), "1.2.3.130@o2ib.stats=Y\n"))
        )
    }
}

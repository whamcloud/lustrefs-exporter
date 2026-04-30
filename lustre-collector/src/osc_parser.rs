// Copyright (c) 2026 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    base_parsers::till_period,
    types::{
        Controller, ControllerStat, ControllerStats, ControllerVariant, OscState, Param, Record,
    },
};
use combine::{
    Parser, attempt,
    error::{ParseError, StreamError},
    many1,
    parser::{
        char::{newline, string},
        token::satisfy,
    },
    stream::{Stream, StreamErrorFor},
};

pub(crate) const OSC: &str = "osc";
pub(crate) const STATE: &str = "state";

pub(crate) fn params() -> Vec<String> {
    vec![format!("{OSC}.*.{STATE}")]
}

#[derive(Debug)]
enum OscStat {
    State(OscState),
}

fn controller_and_variant<I>() -> impl Parser<I, Output = (Controller, ControllerVariant)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (attempt(string("osc.")), till_period(), string(".state="))
        .map(|(_, name, _): (_, String, _)| {
            // Extract controller name from "osc.<name>.state=" pattern
            // OSC controllers always have ControllerVariant::Osc
            (Controller(name), ControllerVariant::Osc)
        })
        .message("while parsing controller_and_variant")
}

/// Reads lines until the next line contains '=' (indicating next param) or input ends
/// Returns all read lines as a single string
fn read_until_next_param<I>() -> impl Parser<I, Output = String>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    // Read one line (everything until newline)
    let read_line = many1::<String, _, _>(satisfy(|c| c != '\n'))
        .skip(newline())
        .map(|s: String| format!("{}\n", s));

    // Keep reading lines until we see a line containing '=' or EOF
    // A line containing '=' indicates the start of the next parameter
    combine::parser::repeat::repeat_until(
        read_line,
        combine::look_ahead(
            attempt(
                combine::parser::repeat::skip_many(satisfy(|c: char| c != '\n' && c != '='))
                    .with(string("=")),
            )
            .map(|_| ()),
        )
        .or(combine::eof().map(|_| ())),
    )
    .map(|lines: Vec<String>| lines.join(""))
}

fn osc_state<I>() -> impl Parser<I, Output = OscState>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    newline()
        .with(read_until_next_param())
        .and_then(|yaml_str: String| -> Result<OscState, StreamErrorFor<I>> {
            // Parse YAML to extract current_state
            // ControllerState's Deserialize implementation handles the conversion
            yaml_serde::from_str(&yaml_str).map_err(StreamErrorFor::<I>::other)
        })
        .message("while parsing osc state")
}

pub(crate) fn parse<I>() -> impl Parser<I, Output = Record>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (controller_and_variant(), osc_state().map(OscStat::State))
        .map(|((controller, kind), stat)| match stat {
            OscStat::State(value) => ControllerStats::OscState(ControllerStat {
                kind,
                param: Param(STATE.to_string()),
                controller,
                value,
            }),
        })
        .map(Record::Controller)
        .message("while parsing osc param")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::ControllerState;
    use combine::EasyParser;

    #[test]
    fn test_parse_osc_state() {
        let input = r#"osc.fs-OST0000-osc-MDT0000.state=
current_state: FULL
state_history:
 - [ 1775627216, CONNECTING ]
 - [ 1775627216, FULL ]
"#;

        let result = parse().easy_parse(input);
        assert!(result.is_ok(), "Parse failed: {:?}", result);

        let (record, _) = result.unwrap();
        match record {
            Record::Controller(ControllerStats::OscState(stat)) => {
                assert_eq!(stat.controller.0, "fs-OST0000-osc-MDT0000");
                assert_eq!(stat.value.current_state, ControllerState::Full);
            }
            _ => panic!("Expected OscState record"),
        }
    }

    #[test]
    fn test_params() {
        let params = params();
        assert_eq!(params, vec!["osc.*.state"]);
    }
}

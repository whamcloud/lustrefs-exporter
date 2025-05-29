// Copyright (c) 2024 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    LustreServiceStats, Record,
    base_parsers::{param, period},
    ldlm::LDLM,
    stats_parser::stats,
};
use combine::{ParseError, Parser, Stream, attempt, choice, parser::char::string};

pub(crate) const LDLM_CANCELD: &str = "ldlm_canceld";
pub(crate) const LDLM_CBD: &str = "ldlm_cbd";

pub(crate) const SERVICES: &str = "services";

pub(crate) const STATS: &str = "stats";

pub(crate) fn params() -> Vec<String> {
    [LDLM_CANCELD, LDLM_CBD]
        .into_iter()
        .map(|x| format!("{LDLM}.{SERVICES}.{x}.{STATS}"))
        .collect()
}

pub(crate) fn parse<I>() -> impl Parser<I, Output = Record>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    attempt((string(SERVICES), period()))
        .with(choice((ldlm_canceld_parser(), ldlm_cbd_parser())))
        .map(Record::LustreService)
}

pub(crate) fn ldlm_canceld_parser<I>() -> impl Parser<I, Output = LustreServiceStats>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    attempt((string(LDLM_CANCELD), period(), param(STATS)))
        .with(stats())
        .map(LustreServiceStats::LdlmCanceld)
        .message("While parsing ldlm_canceld.stats")
}

pub(crate) fn ldlm_cbd_parser<I>() -> impl Parser<I, Output = LustreServiceStats>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (string(LDLM_CBD), period(), param(STATS))
        .with(stats())
        .map(LustreServiceStats::LdlmCbd)
        .message("While parsing ldlm_cbd.stats")
}

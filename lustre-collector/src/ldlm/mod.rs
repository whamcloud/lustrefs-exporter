// Copyright (c) 2024 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{base_parsers::period, Record};
use combine::{attempt, parser::char::string, ParseError, Parser, Stream};

mod ldlm_namespace_parser;
mod ldlm_service_parser;

pub(crate) const LDLM: &str = "ldlm";

pub(crate) fn params() -> Vec<String> {
    ldlm_namespace_parser::params()
        .into_iter()
        .chain(ldlm_service_parser::params())
        .collect()
}

pub(crate) fn parse<'a, I>() -> impl Parser<I, Output = Record<'a>>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (attempt(string(LDLM)), period())
        .with(ldlm_namespace_parser::parse().or(ldlm_service_parser::parse()))
        .message("while parsing ldlm")
}

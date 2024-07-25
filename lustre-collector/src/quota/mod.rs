// Copyright (c) 2024 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{base_parsers::period, Record};
use combine::{parser::char::string, ParseError, Parser, Stream};

pub(crate) mod quota_parser;

pub(crate) const QMT: &str = "qmt";

pub(crate) fn params() -> Vec<String> {
    quota_parser::params()
}

pub(crate) fn parse<'a, I>() -> impl Parser<I, Output = Record<'a>>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (string(QMT), period()).with(quota_parser::qmt_parse())
}

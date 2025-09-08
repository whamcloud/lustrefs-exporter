// Copyright (c) 2024 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{Record, base_parsers::period};
use combine::{ParseError, Parser, Stream, parser::char::string};

pub(crate) mod quota_parser;

pub(crate) const QMT: &str = "qmt";

/// Takes QMT_STATS and produces a list of params for
/// consumption in proper ltcl get_param format.
pub(crate) const fn params() -> [&'static str; 3] {
    ["qmt.*.*.glb-usr", "qmt.*.*.glb-prj", "qmt.*.*.glb-grp"]
}

pub fn parse<I>() -> impl Parser<I, Output = Record>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (string(QMT), period()).with(quota_parser::qmt_parse())
}

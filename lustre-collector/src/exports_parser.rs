// Copyright (c) 2024 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    ExportStats,
    base_parsers::{equals, period},
    stats_parser::stats,
};
use combine::{
    Parser, attempt,
    error::ParseError,
    many, many1,
    parser::char::{alpha_num, string},
    stream::Stream,
    token,
};

/// Parses a single nid
pub(crate) fn nid<I>() -> impl Parser<I, Output = String>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        many1::<String, _, _>(alpha_num().or(period()).or(token(':'))),
        token('@'),
        many1::<String, _, _>(alpha_num()),
    )
        .map(|(ip, _, lnet)| format!("{ip}@{lnet}"))
        .message("while parsing nid")
}

/// Parses a single obdfilter.*OST*.exports.*.stats line
fn exports_stat<I>() -> impl Parser<I, Output = ExportStats>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    attempt((
        nid().skip(period()),
        string("stats").skip(equals()),
        stats(),
    ))
    .map(|(nid, _, stats)| ExportStats { nid, stats })
    .message("while parsing export_stats")
}

/// Parses multiple obdfilter.*OST*.exports.*.stats lines
pub(crate) fn exports_stats<I>() -> impl Parser<I, Output = Vec<ExportStats>>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (many(exports_stat())).map(|x| x)
}

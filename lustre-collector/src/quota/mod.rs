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

pub use quota_parser::w::parse_all as w_parse;

mod test {
    #[test]
    fn parse_quotas() {
        use crate::quota::{parse as combine_parse, quota_parser::w::parse_all as winnow_parse};
        use combine::Parser as _;
        use std::{fs::File, io::Read};
        use winnow::Parser;

        let mut raw = String::new();

        File::open("benches/quotas.yml")
            .unwrap()
            .read_to_string(&mut raw)
            .unwrap();

        let anchor = std::time::Instant::now();
        let mut needle = raw.as_str();
        let mut combine_out = Vec::new();
        while let Ok((t, e)) = combine_parse().parse(needle) {
            combine_out.push(t);
            needle = e;
        }
        println!("Elapsed in combine: {}s", anchor.elapsed().as_secs());

        let anchor = std::time::Instant::now();
        let needle = &mut raw.as_str();
        let winnow_out = winnow_parse
            .parse(needle)
            .inspect_err(|e| println!("{e}"))
            .unwrap();
        println!("Elapsed in winnow: {}ms", anchor.elapsed().as_millis());

        assert_eq!(combine_out, winnow_out)
    }
}

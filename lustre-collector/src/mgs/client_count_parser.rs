// Copyright (c) 2021 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    base_parsers::period,
    mds::client_count_parser::{exports, is_client},
    types::{Param, Record, Target, TargetStat, TargetStats, TargetVariant},
};
use combine::{
    attempt, choice,
    error::ParseError,
    many1, one_of,
    parser::char::{alpha_num, newline, string},
    sep_end_by,
    stream::Stream,
    Parser,
};
use std::{collections::BTreeMap, ops::Add};

pub(crate) const EXPORTS: &str = "exports";

pub(crate) fn params() -> Vec<String> {
    vec![format!("mgs.*.{}.*.uuid", EXPORTS)]
}

pub(crate) fn parse<I>() -> impl Parser<I, Output = Vec<Record>>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    many1(interface_clients())
        .map(|xs: Vec<_>| {
            xs.into_iter().fold(BTreeMap::new(), |mut acc, (k, v)| {
                acc.entry(k).and_modify(|x| *x += v).or_insert(v);

                acc
            })
        })
        .map(|hm| {
            hm.into_iter()
                .map(|(k, value)| TargetStat {
                    kind: TargetVariant::Mgt,
                    target: Target(k),
                    param: Param("connected_clients".into()),
                    value,
                })
                .map(TargetStats::ConnectedClients)
                .map(Record::Target)
                .collect()
        })
}

fn interface_clients<I>() -> impl Parser<I, Output = (String, u64)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        attempt(obdfilter_interface()),
        choice((
            newline()
                .with(sep_end_by(attempt(is_client()), newline()))
                .map(|xs: Vec<_>| xs.into_iter().fold(0, Add::add)),
            attempt(is_client()).skip(newline()),
        )),
    )
}

fn obdfilter_interface<I>() -> impl Parser<I, Output = String>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        string("mgs").skip(period()),
        many1::<String, _, _>(alpha_num().or(one_of("_-".chars()))),
        period().skip(exports()),
    )
        .map(|(_, x, _)| x)
}

#[cfg(test)]
mod test {
    use super::*;
    use combine::parser::EasyParser;
    use insta::assert_debug_snapshot;

    #[test]
    fn test_is_client() {
        let result = is_client()
            .easy_parse("a01e9c48-52f7-0c50-ff15-5aa13684bb5b\n")
            .unwrap();

        assert!(result.0 == 1)
    }

    #[test]
    fn test_is_not_client() {
        let result = is_client().parse("fs-MDT0001-mdtlov_UUID\n").unwrap();

        assert!(result.0 == 0)
    }

    #[test]
    fn test_export_param() {
        let result: (String, &'static str) = obdfilter_interface()
            .easy_parse("mgs.MGS.exports.0@lo.uuid=")
            .unwrap();

        assert_debug_snapshot!(result)
    }

    #[test]
    fn test_interface_clients() {
        let result = interface_clients()
            .easy_parse(
                "mgs.MGS.exports.10.73.20.12@tcp.uuid=1debcd80-d3b4-414d-b872-4075bd4f2340\n",
            )
            .unwrap();

        assert_debug_snapshot!(result)
    }

    #[test]
    fn test_client_count_parser_one_client() {
        let x = r#"mgs.MGS.exports.0@lo.uuid=906485f9-b487-4f5c-96ab-7c54e4c94d16
mgs.MGS.exports.10.73.20.12@tcp.uuid=1debcd80-d3b4-414d-b872-4075bd4f2340
mgs.MGS.exports.10.73.20.13@tcp.uuid=645be8f4-2312-441e-8f62-e06d58e896f3
mgs.MGS.exports.10.73.20.14@tcp.uuid=e1510c7d-e048-438a-ae52-b583b518dcc3
mgs.MGS.exports.10.73.20.15@tcp.uuid=a7367d8c-9a7e-4ad0-97c7-9d2c12e3b0a3
mgs.MGS.exports.10.73.20.16@tcp.uuid=fe449dca-2354-4eb6-aecb-af91aac0933b
mgs.MGS.exports.10.73.20.17@tcp.uuid=693f73f7-4300-4f25-b913-602797ccd0b7
mgs.MGS.exports.10.73.20.18@tcp.uuid=2abee679-6e82-4f88-ae39-79ed10fc0a82
mgs.MGS.exports.10.73.20.2@tcp.uuid=9c61f51f-5d57-4e87-916c-d551e5497622
"#;

        let result = parse().easy_parse(x).unwrap();

        assert_debug_snapshot!(result)
    }

    #[test]
    fn test_client_count_parser_two_client() {
        let x = r#"mgs.MGS.exports.0@lo.uuid=906485f9-b487-4f5c-96ab-7c54e4c94d16
mgs.MGS.exports.10.73.20.12@tcp.uuid=1debcd80-d3b4-414d-b872-4075bd4f2340
mgs.MGS.exports.10.73.20.13@tcp.uuid=645be8f4-2312-441e-8f62-e06d58e896f3
mgs.MGS.exports.10.73.20.14@tcp.uuid=e1510c7d-e048-438a-ae52-b583b518dcc3
mgs.MGS.exports.10.73.20.15@tcp.uuid=a7367d8c-9a7e-4ad0-97c7-9d2c12e3b0a3
mgs.MGS.exports.10.73.20.16@tcp.uuid=fe449dca-2354-4eb6-aecb-af91aac0933b
mgs.MGS.exports.10.73.20.17@tcp.uuid=693f73f7-4300-4f25-b913-602797ccd0b7
mgs.MGS.exports.10.73.20.18@tcp.uuid=2abee679-6e82-4f88-ae39-79ed10fc0a82
mgs.MGS.exports.10.73.20.2@tcp.uuid=9c61f51f-5d57-4e87-916c-d551e5497622
mgs.MGS.exports.10.73.20.3@tcp.uuid=41a343e1-c8d9-4df8-89d3-df433d5e8bd8
"#;

        let result = parse().easy_parse(x).unwrap();

        assert_debug_snapshot!(result)
    }

    #[test]
    fn test_client_count_parser_zero_clients() {
        let x = r#"mgs.MGS.exports.0@lo.uuid=906485f9-b487-4f5c-96ab-7c54e4c94d16
mgs.MGS.exports.10.73.20.12@tcp.uuid=1debcd80-d3b4-414d-b872-4075bd4f2340
mgs.MGS.exports.10.73.20.13@tcp.uuid=645be8f4-2312-441e-8f62-e06d58e896f3
mgs.MGS.exports.10.73.20.14@tcp.uuid=e1510c7d-e048-438a-ae52-b583b518dcc3
mgs.MGS.exports.10.73.20.15@tcp.uuid=a7367d8c-9a7e-4ad0-97c7-9d2c12e3b0a3
mgs.MGS.exports.10.73.20.16@tcp.uuid=fe449dca-2354-4eb6-aecb-af91aac0933b
mgs.MGS.exports.10.73.20.17@tcp.uuid=693f73f7-4300-4f25-b913-602797ccd0b7
mgs.MGS.exports.10.73.20.18@tcp.uuid=2abee679-6e82-4f88-ae39-79ed10fc0a82
"#;

        let result = parse().easy_parse(x).unwrap();

        assert_debug_snapshot!(result)
    }
}

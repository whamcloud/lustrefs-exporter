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
    vec![format!("obdfilter.*.{}.*.uuid", EXPORTS)]
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
                    kind: TargetVariant::Ost,
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
        string("obdfilter").skip(period()),
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
        let result = obdfilter_interface()
            .easy_parse("obdfilter.fs-OST0000.exports.0@lo.uuid=")
            .unwrap();

        assert_debug_snapshot!(result)
    }

    #[test]
    fn test_no_interface_clients() {
        let result = interface_clients()
            .easy_parse(
                "obdfilter.fs-OST0000.exports.10.73.20.12@tcp.uuid=fs-MDT0001-mdtlov_UUID\n",
            )
            .unwrap();

        assert_debug_snapshot!(result)
    }

    #[test]
    fn test_interface_clients() {
        let result = interface_clients()
            .easy_parse("obdfilter.fs-OST0000.exports.10.73.20.2@tcp.uuid=0223192c-b9e4-49b3-b09e-d176a5e87f6e\n")
            .unwrap();

        assert_debug_snapshot!(result)
    }

    #[test]
    fn test_client_count_parser_one_client() {
        let x = r#"obdfilter.fs-OST0000.exports.0@lo.uuid=fs-MDT0000-mdtlov_UUID
obdfilter.fs-OST0000.exports.10.73.20.12@tcp.uuid=fs-MDT0001-mdtlov_UUID
obdfilter.fs-OST0000.exports.10.73.20.13@tcp.uuid=fs-MDT0002-mdtlov_UUID
obdfilter.fs-OST0000.exports.10.73.20.14@tcp.uuid=fs-MDT0003-mdtlov_UUID
obdfilter.fs-OST0000.exports.10.73.20.15@tcp.uuid=fs-MDT0004-mdtlov_UUID
obdfilter.fs-OST0000.exports.10.73.20.16@tcp.uuid=fs-MDT0005-mdtlov_UUID
obdfilter.fs-OST0000.exports.10.73.20.17@tcp.uuid=fs-MDT0006-mdtlov_UUID
obdfilter.fs-OST0000.exports.10.73.20.18@tcp.uuid=fs-MDT0007-mdtlov_UUID
obdfilter.fs-OST0000.exports.10.73.20.2@tcp.uuid=0223192c-b9e4-49b3-b09e-d176a5e87f6e
"#;

        let result = parse().easy_parse(x).unwrap();

        assert_debug_snapshot!(result)
    }

    #[test]
    fn test_client_count_parser_two_client() {
        let x = r#"obdfilter.fs-OST0000.exports.0@lo.uuid=fs-MDT0000-mdtlov_UUID
obdfilter.fs-OST0000.exports.10.73.20.12@tcp.uuid=fs-MDT0001-mdtlov_UUID
obdfilter.fs-OST0000.exports.10.73.20.13@tcp.uuid=fs-MDT0002-mdtlov_UUID
obdfilter.fs-OST0000.exports.10.73.20.14@tcp.uuid=fs-MDT0003-mdtlov_UUID
obdfilter.fs-OST0000.exports.10.73.20.15@tcp.uuid=fs-MDT0004-mdtlov_UUID
obdfilter.fs-OST0000.exports.10.73.20.16@tcp.uuid=fs-MDT0005-mdtlov_UUID
obdfilter.fs-OST0000.exports.10.73.20.17@tcp.uuid=fs-MDT0006-mdtlov_UUID
obdfilter.fs-OST0000.exports.10.73.20.18@tcp.uuid=fs-MDT0007-mdtlov_UUID
obdfilter.fs-OST0000.exports.10.73.20.2@tcp.uuid=82ccc63d-c30e-4ab1-b71b-5e57b37aba02
obdfilter.fs-OST0000.exports.10.73.20.3@tcp.uuid=4cf9b354-ce92-4bf4-ab6c-015a5f5d1f0c
obdfilter.fs-OST0001.exports.0@lo.uuid=fs-MDT0000-mdtlov_UUID
obdfilter.fs-OST0001.exports.10.73.20.12@tcp.uuid=fs-MDT0001-mdtlov_UUID
obdfilter.fs-OST0001.exports.10.73.20.13@tcp.uuid=fs-MDT0002-mdtlov_UUID
obdfilter.fs-OST0001.exports.10.73.20.14@tcp.uuid=fs-MDT0003-mdtlov_UUID
obdfilter.fs-OST0001.exports.10.73.20.15@tcp.uuid=fs-MDT0004-mdtlov_UUID
obdfilter.fs-OST0001.exports.10.73.20.16@tcp.uuid=fs-MDT0005-mdtlov_UUID
obdfilter.fs-OST0001.exports.10.73.20.17@tcp.uuid=fs-MDT0006-mdtlov_UUID
obdfilter.fs-OST0001.exports.10.73.20.18@tcp.uuid=fs-MDT0007-mdtlov_UUID
obdfilter.fs-OST0001.exports.10.73.20.2@tcp.uuid=82ccc63d-c30e-4ab1-b71b-5e57b37aba02
obdfilter.fs-OST0001.exports.10.73.20.3@tcp.uuid=4cf9b354-ce92-4bf4-ab6c-015a5f5d1f0c
"#;

        let result = parse().easy_parse(x).unwrap();

        assert_debug_snapshot!(result)
    }

    #[test]
    fn test_client_count_parser_zero_clients() {
        let x = r#"obdfilter.fs-OST0000.exports.0@lo.uuid=fs-MDT0000-mdtlov_UUID
obdfilter.fs-OST0000.exports.10.73.20.12@tcp.uuid=fs-MDT0001-mdtlov_UUID
obdfilter.fs-OST0000.exports.10.73.20.13@tcp.uuid=fs-MDT0002-mdtlov_UUID
obdfilter.fs-OST0000.exports.10.73.20.14@tcp.uuid=fs-MDT0003-mdtlov_UUID
obdfilter.fs-OST0000.exports.10.73.20.15@tcp.uuid=fs-MDT0004-mdtlov_UUID
obdfilter.fs-OST0000.exports.10.73.20.16@tcp.uuid=fs-MDT0005-mdtlov_UUID
obdfilter.fs-OST0000.exports.10.73.20.17@tcp.uuid=fs-MDT0006-mdtlov_UUID
obdfilter.fs-OST0000.exports.10.73.20.18@tcp.uuid=fs-MDT0007-mdtlov_UUID
obdfilter.fs-OST0001.exports.0@lo.uuid=fs-MDT0000-mdtlov_UUID
obdfilter.fs-OST0001.exports.10.73.20.12@tcp.uuid=fs-MDT0001-mdtlov_UUID
obdfilter.fs-OST0001.exports.10.73.20.13@tcp.uuid=fs-MDT0002-mdtlov_UUID
obdfilter.fs-OST0001.exports.10.73.20.14@tcp.uuid=fs-MDT0003-mdtlov_UUID
obdfilter.fs-OST0001.exports.10.73.20.15@tcp.uuid=fs-MDT0004-mdtlov_UUID
obdfilter.fs-OST0001.exports.10.73.20.16@tcp.uuid=fs-MDT0005-mdtlov_UUID
obdfilter.fs-OST0001.exports.10.73.20.17@tcp.uuid=fs-MDT0006-mdtlov_UUID
obdfilter.fs-OST0001.exports.10.73.20.18@tcp.uuid=fs-MDT0007-mdtlov_UUID
"#;

        let result = parse().easy_parse(x).unwrap();

        assert_debug_snapshot!(result)
    }

    #[test]
    fn test_client_count_parser_multiple_fs() {
        let x = r#"obdfilter.fs-OST0000.exports.0@lo.uuid=fs-MDT0000-mdtlov_UUID
obdfilter.fs-OST0000.exports.10.73.20.12@tcp.uuid=fs-MDT0001-mdtlov_UUID
obdfilter.fs-OST0000.exports.10.73.20.13@tcp.uuid=fs-MDT0002-mdtlov_UUID
obdfilter.fs-OST0000.exports.10.73.20.14@tcp.uuid=fs-MDT0003-mdtlov_UUID
obdfilter.fs-OST0000.exports.10.73.20.15@tcp.uuid=fs-MDT0004-mdtlov_UUID
obdfilter.fs-OST0000.exports.10.73.20.16@tcp.uuid=fs-MDT0005-mdtlov_UUID
obdfilter.fs-OST0000.exports.10.73.20.17@tcp.uuid=fs-MDT0006-mdtlov_UUID
obdfilter.fs-OST0000.exports.10.73.20.18@tcp.uuid=fs-MDT0007-mdtlov_UUID
obdfilter.fs-OST0000.exports.10.73.20.2@tcp.uuid=82ccc63d-c30e-4ab1-b71b-5e57b37aba02
obdfilter.fs-OST0001.exports.0@lo.uuid=fs-MDT0000-mdtlov_UUID
obdfilter.fs-OST0001.exports.10.73.20.12@tcp.uuid=fs-MDT0001-mdtlov_UUID
obdfilter.fs-OST0001.exports.10.73.20.13@tcp.uuid=fs-MDT0002-mdtlov_UUID
obdfilter.fs-OST0001.exports.10.73.20.14@tcp.uuid=fs-MDT0003-mdtlov_UUID
obdfilter.fs-OST0001.exports.10.73.20.15@tcp.uuid=fs-MDT0004-mdtlov_UUID
obdfilter.fs-OST0001.exports.10.73.20.16@tcp.uuid=fs-MDT0005-mdtlov_UUID
obdfilter.fs-OST0001.exports.10.73.20.17@tcp.uuid=fs-MDT0006-mdtlov_UUID
obdfilter.fs-OST0001.exports.10.73.20.18@tcp.uuid=fs-MDT0007-mdtlov_UUID
obdfilter.fs-OST0001.exports.10.73.20.2@tcp.uuid=82ccc63d-c30e-4ab1-b71b-5e57b37aba02
obdfilter.fs2-OST0000.exports.0@lo.uuid=fs2-MDT0000-mdtlov_UUID
obdfilter.fs2-OST0000.exports.10.73.20.12@tcp.uuid=fs2-MDT0001-mdtlov_UUID
obdfilter.fs2-OST0000.exports.10.73.20.13@tcp.uuid=fs2-MDT0002-mdtlov_UUID
obdfilter.fs2-OST0000.exports.10.73.20.14@tcp.uuid=fs2-MDT0003-mdtlov_UUID
obdfilter.fs2-OST0000.exports.10.73.20.15@tcp.uuid=fs2-MDT0004-mdtlov_UUID
obdfilter.fs2-OST0000.exports.10.73.20.16@tcp.uuid=fs2-MDT0005-mdtlov_UUID
obdfilter.fs2-OST0000.exports.10.73.20.17@tcp.uuid=fs2-MDT0006-mdtlov_UUID
obdfilter.fs2-OST0000.exports.10.73.20.18@tcp.uuid=fs2-MDT0007-mdtlov_UUID
obdfilter.fs2-OST0000.exports.10.73.20.2@tcp.uuid=82ccc63d-c30e-4ab1-b71b-5e57b37aba02
obdfilter.fs2-OST0001.exports.0@lo.uuid=fs2-MDT0000-mdtlov_UUID
obdfilter.fs2-OST0001.exports.10.73.20.12@tcp.uuid=fs2-MDT0001-mdtlov_UUID
obdfilter.fs2-OST0001.exports.10.73.20.13@tcp.uuid=fs2-MDT0002-mdtlov_UUID
obdfilter.fs2-OST0001.exports.10.73.20.14@tcp.uuid=fs2-MDT0003-mdtlov_UUID
obdfilter.fs2-OST0001.exports.10.73.20.15@tcp.uuid=fs2-MDT0004-mdtlov_UUID
obdfilter.fs2-OST0001.exports.10.73.20.16@tcp.uuid=fs2-MDT0005-mdtlov_UUID
obdfilter.fs2-OST0001.exports.10.73.20.17@tcp.uuid=fs2-MDT0006-mdtlov_UUID
obdfilter.fs2-OST0001.exports.10.73.20.18@tcp.uuid=fs2-MDT0007-mdtlov_UUID
obdfilter.fs2-OST0001.exports.10.73.20.2@tcp.uuid=82ccc63d-c30e-4ab1-b71b-5e57b37aba02
"#;

        let result = parse().easy_parse(x).unwrap();

        assert_debug_snapshot!(result)
    }
}

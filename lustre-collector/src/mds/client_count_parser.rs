// Copyright (c) 2021 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    base_parsers::{equals, period},
    exports_parser::nid,
    types::{Param, Record, Target, TargetStat, TargetStats, TargetVariant},
};
use combine::{
    Parser, attempt, choice,
    error::ParseError,
    look_ahead, many1, one_of,
    parser::char::{alpha_num, newline, string},
    sep_by1, sep_end_by,
    stream::Stream,
    token,
};
use std::{collections::BTreeMap, ops::Add};

pub(crate) const EXPORTS: &str = "exports";

pub(crate) fn params() -> Vec<String> {
    vec![
        format!("mdt.*.{}.*.uuid", EXPORTS),
        format!("obdfilter.*.{}.*.uuid", EXPORTS),
        format!("mgs.MGS.{}.*.uuid", EXPORTS),
    ]
}

pub(crate) fn parse<I>() -> impl Parser<I, Output = Vec<Record>>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    many1(interface_clients())
        .map(|xs: Vec<_>| {
            xs.into_iter().fold(BTreeMap::new(), |mut acc, ((target, kind), count)| {
                acc.entry((target, kind)).and_modify(|x| *x += count).or_insert(count);

                acc
            })
        })
        .map(|hm| {
            hm.into_iter()
                .map(|((target, kind), value)| TargetStat {
                    kind,
                    target: Target(target),
                    param: Param("connected_clients".into()),
                    value,
                })
                .map(TargetStats::ConnectedClients)
                .map(Record::Target)
                .collect()
        })
}

fn interface_clients<I>() -> impl Parser<I, Output = ((String, TargetVariant), u64)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        choice((
            attempt(mdt_interface()),
            attempt(ost_interface()),
            attempt(mgt_interface()),
        )),
        choice((
            newline()
                .with(sep_end_by(attempt(is_client()), newline()))
                .map(|xs: Vec<_>| xs.into_iter().fold(0, Add::add)),
            attempt(is_client()).skip(newline()),
        )),
    )
}

fn is_client<I>() -> impl Parser<I, Output = u64>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    sep_by1::<Vec<_>, _, _, _>(many1::<String, _, _>(alpha_num()), token('-')).with(choice((
        string("_UUID").map(|_| 0),
        look_ahead(newline()).map(|_| 1),
    )))
}

pub(crate) fn exports<I>() -> impl Parser<I, Output = ()>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        string("exports"),
        period(),
        nid(),
        period(),
        string("uuid"),
        equals(),
    )
        .map(drop)
}

fn mdt_interface<I>() -> impl Parser<I, Output = (String, TargetVariant)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        string("mdt").skip(period()),
        many1::<String, _, _>(alpha_num().or(one_of("_-".chars()))),
        period().skip(exports()),
    )
        .map(|(_, x, _)| (x, TargetVariant::Mdt))
}

fn ost_interface<I>() -> impl Parser<I, Output = (String, TargetVariant)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        string("obdfilter").skip(period()),
        many1::<String, _, _>(alpha_num().or(one_of("_-".chars()))),
        period().skip(exports()),
    )
        .map(|(_, x, _)| (x, TargetVariant::Ost))
}

fn mgt_interface<I>() -> impl Parser<I, Output = (String, TargetVariant)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        string("mgs").skip(period()),
        string("MGS"),
        period().skip(exports()),
    )
        .map(|(_, x, _)| (x.to_string(), TargetVariant::Mgt))
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

        assert_debug_snapshot!(result)
    }

    #[test]
    fn test_is_not_client() {
        let result = is_client()
            .parse("es01a-MDT0000-lwp-OST0000_UUID\n")
            .unwrap();

        assert_debug_snapshot!(result)
    }

    #[test]
    fn test_export_param() {
        let result = mdt_interface()
            .easy_parse("mdt.es01a-MDT0000.exports.0@lo.uuid=")
            .unwrap();

        assert_debug_snapshot!(result)
    }

    #[test]
    fn test_no_interface_clients() {
        let result = interface_clients()
            .easy_parse("mdt.fs-MDT0000.exports.0@lo.uuid=es01a-MDT0000-lwp-MDT0000_UUID\n")
            .unwrap();

        assert_debug_snapshot!(result)
    }

    #[test]
    fn test_interface_clients() {
        let result = interface_clients()
            .easy_parse("mdt.fs-MDT0000.exports.0@lo.uuid=a01e9c48-52f7-0c50-ff15-5aa13684bb5b\n")
            .unwrap();

        assert_debug_snapshot!(result)
    }

    #[test]
    fn test_multiple_interface_clients() {
        let x = r#"mdt.fs-MDT0000.exports.0@lo.uuid=
es01a-MDT0000-lwp-OST0002_UUID
a01e9c48-52f7-0c50-ff15-5aa13684bb5a
es01a-MDT0000-lwp-OST0001_UUID
a01e9c48-52f7-0c50-ff15-5aa13684bb5b
es01a-MDT0000-lwp-OST0000_UUID
a01e9c48-52f7-0c50-ff15-5aa13684bb5c
es01a-MDT0000-lwp-OST0000_UUID
es01a-MDT0000-lwp-OST0000_UUID
a01e9c48-52f7-0c50-ff15-5aa13684bb5c
a01e9c48-52f7-0c50-ff15-5aa13684bb5c
"#;

        let result = interface_clients().easy_parse(x).unwrap();

        assert_debug_snapshot!(result)
    }

    #[test]
    fn test_client_count_parser_one_client() {
        let x = r#"mdt.fs-MDT0000.exports.0@lo.uuid=es01a-MDT0000-lwp-MDT0000_UUID
mdt.es01a-MDT0000.exports.172.60.0.2@o2ib.uuid=
es01a-MDT0000-lwp-OST0002_UUID
es01a-MDT0000-lwp-OST0001_UUID
es01a-MDT0000-lwp-OST0000_UUID
mdt.es01a-MDT0000.exports.172.60.0.4@o2ib.uuid=es01a-MDT0000-lwp-OST0003_UUID
mdt.es01a-MDT0000.exports.172.60.14.106@o2ib.uuid=
a01e9c48-52f7-0c50-ff15-5aa13684bb5b
"#;

        let result = parse().easy_parse(x).unwrap();

        assert_debug_snapshot!(result)
    }

    #[test]
    fn test_client_count_parser_zero_clients() {
        let x = r#"mdt.fs-MDT0000.exports.0@lo.uuid=fs-MDT0000-lwp-MDT0000_UUID
mdt.fs-MDT0000.exports.10.73.20.21@tcp.uuid=
fs-MDT0000-lwp-OST000a_UUID
fs-MDT0000-lwp-OST0004_UUID
fs-MDT0000-lwp-OST0000_UUID
fs-MDT0000-lwp-OST0006_UUID
fs-MDT0000-lwp-OST0002_UUID
fs-MDT0000-lwp-OST0008_UUID
mdt.fs-MDT0000.exports.10.73.20.22@tcp.uuid=
fs-MDT0000-lwp-OST0007_UUID
fs-MDT0000-lwp-OST0003_UUID
fs-MDT0000-lwp-OST0009_UUID
fs-MDT0000-lwp-OST0001_UUID
fs-MDT0000-lwp-OST000b_UUID
fs-MDT0000-lwp-OST0005_UUID
"#;

        let result = parse().easy_parse(x).unwrap();

        assert_debug_snapshot!(result)
    }

    #[test]
    fn test_client_count_parser_two_clients() {
        let x = r#"mdt.fs-MDT0000.exports.0@lo.uuid=fs-MDT0000-lwp-MDT0000_UUID
mdt.fs-MDT0000.exports.10.0.2.15@tcp.uuid=
613beb43-5df2-2ace-4209-be66b4b509df
568acc64-085e-ada1-d493-6318930dfa74
mdt.fs-MDT0000.exports.10.73.20.21@tcp.uuid=
fs-MDT0000-lwp-OST000a_UUID
fs-MDT0000-lwp-OST0004_UUID
fs-MDT0000-lwp-OST0000_UUID
fs-MDT0000-lwp-OST0006_UUID
fs-MDT0000-lwp-OST0002_UUID
fs-MDT0000-lwp-OST0008_UUID
mdt.fs-MDT0000.exports.10.73.20.22@tcp.uuid=
fs-MDT0000-lwp-OST0007_UUID
fs-MDT0000-lwp-OST0003_UUID
fs-MDT0000-lwp-OST0009_UUID
fs-MDT0000-lwp-OST0001_UUID
fs-MDT0000-lwp-OST000b_UUID
fs-MDT0000-lwp-OST0005_UUID
"#;

        let result = parse().easy_parse(x).unwrap();

        assert_debug_snapshot!(result)
    }

    #[test]
    fn test_client_count_parser_multiple_fs() {
        let x = r#"mdt.fs-MDT0000.exports.0@lo.uuid=fs-MDT0000-lwp-MDT0000_UUID
mdt.fs-MDT0000.exports.10.0.2.15@tcp.uuid=
613beb43-5df2-2ace-4209-be66b4b509df
568acc64-085e-ada1-d493-6318930dfa74
mdt.fs-MDT0000.exports.10.73.20.21@tcp.uuid=
fs-MDT0000-lwp-OST000a_UUID
fs-MDT0000-lwp-OST0004_UUID
fs-MDT0000-lwp-OST0000_UUID
fs-MDT0000-lwp-OST0006_UUID
fs-MDT0000-lwp-OST0002_UUID
fs-MDT0000-lwp-OST0008_UUID
mdt.fs-MDT0000.exports.10.73.20.22@tcp.uuid=
fs-MDT0000-lwp-OST0007_UUID
fs-MDT0000-lwp-OST0003_UUID
fs-MDT0000-lwp-OST0009_UUID
fs-MDT0000-lwp-OST0001_UUID
fs-MDT0000-lwp-OST000b_UUID
fs-MDT0000-lwp-OST0005_UUID
mdt.fs2-MDT0000.exports.0@lo.uuid=fs2-MDT0000-lwp-MDT0000_UUID
mdt.fs2-MDT0000.exports.10.0.2.15@tcp.uuid=
a7b7c685-18c1-eecc-eae2-5880f431cae3
6f2afad1-ff3a-cdd0-721f-dcc123cae427
mdt.fs2-MDT0000.exports.10.73.20.12@tcp.uuid=fs2-MDT0000-lwp-OST0002_UUID
mdt.fs2-MDT0000.exports.10.73.20.21@tcp.uuid=
fs2-MDT0000-lwp-OST0003_UUID
fs2-MDT0000-lwp-OST0000_UUID
mdt.fs2-MDT0000.exports.10.73.20.22@tcp.uuid=fs2-MDT0000-lwp-OST0001_UUID
"#;

        let result = parse().easy_parse(x).unwrap();

        assert_debug_snapshot!(result)
    }

    #[test]
    fn test_ost_interface() {
        let result = ost_interface()
            .easy_parse("obdfilter.fs-OST0000.exports.0@lo.uuid=")
            .unwrap();

        assert_debug_snapshot!(result)
    }

    #[test]
    fn test_ost_interface_clients() {
        let result = interface_clients()
            .easy_parse("obdfilter.fs-OST0000.exports.0@lo.uuid=a01e9c48-52f7-0c50-ff15-5aa13684bb5b\n")
            .unwrap();

        assert_debug_snapshot!(result)
    }

    #[test]
    fn test_ost_client_count_parser() {
        let x = r#"obdfilter.fs-OST0000.exports.0@lo.uuid=fs-OST0000_UUID
obdfilter.fs-OST0000.exports.10.0.2.15@tcp.uuid=
613beb43-5df2-2ace-4209-be66b4b509df
568acc64-085e-ada1-d493-6318930dfa74
obdfilter.fs-OST0001.exports.0@lo.uuid=fs-OST0001_UUID
obdfilter.fs-OST0001.exports.10.0.2.15@tcp.uuid=
613beb43-5df2-2ace-4209-be66b4b509df
"#;

        let result = parse().easy_parse(x).unwrap();

        assert_debug_snapshot!(result)
    }

    #[test]
    fn test_mgt_interface() {
        let result = mgt_interface()
            .easy_parse("mgs.MGS.exports.0@lo.uuid=")
            .unwrap();

        assert_debug_snapshot!(result)
    }

    #[test]
    fn test_mgt_interface_clients() {
        let result = interface_clients()
            .easy_parse("mgs.MGS.exports.0@lo.uuid=a01e9c48-52f7-0c50-ff15-5aa13684bb5b\n")
            .unwrap();

        assert_debug_snapshot!(result)
    }

    #[test]
    fn test_mgt_client_count_parser() {
        let x = r#"mgs.MGS.exports.0@lo.uuid=MGS_UUID
mgs.MGS.exports.10.0.2.15@tcp.uuid=
613beb43-5df2-2ace-4209-be66b4b509df
568acc64-085e-ada1-d493-6318930dfa74
"#;

        let result = parse().easy_parse(x).unwrap();

        assert_debug_snapshot!(result)
    }

    #[test]
    fn test_mixed_targets_client_count() {
        let x = r#"mdt.fs-MDT0000.exports.0@lo.uuid=fs-MDT0000-lwp-MDT0000_UUID
mdt.fs-MDT0000.exports.10.0.2.15@tcp.uuid=
613beb43-5df2-2ace-4209-be66b4b509df
obdfilter.fs-OST0000.exports.0@lo.uuid=fs-OST0000_UUID
obdfilter.fs-OST0000.exports.10.0.2.15@tcp.uuid=
613beb43-5df2-2ace-4209-be66b4b509df
mgs.MGS.exports.0@lo.uuid=MGS_UUID
mgs.MGS.exports.10.0.2.15@tcp.uuid=
613beb43-5df2-2ace-4209-be66b4b509df
"#;

        let result = parse().easy_parse(x).unwrap();

        assert_debug_snapshot!(result)
    }

    #[test]
    fn test_production_inline_uuid_format() {
        // Real production data showing inline UUID format for OST exports
        // This tests the case where UUIDs appear on the same line as the export path
        let x = r#"obdfilter.sfa18k26-OST0000.exports.0@lo.uuid=sfa18k26-MDT0000-mdtlov_UUID
obdfilter.sfa18k26-OST0000.exports.172.25.80.30@tcp.uuid=326aea2c-009f-400b-b8f9-7df49504fbd0
obdfilter.sfa18k26-OST0000.exports.172.25.80.84@o2ib.uuid=sfa18k26-MDT0001-mdtlov_UUID
obdfilter.sfa18k26-OST0000.exports.172.25.80.90@o2ib.uuid=sfa18k26-MDT0002-mdtlov_UUID
obdfilter.sfa18k26-OST0000.exports.172.25.80.92@o2ib.uuid=sfa18k26-MDT0003-mdtlov_UUID
mgs.MGS.exports.0@lo.uuid=893f59e7-eb98-4898-a972-5f3b8273ed46
mgs.MGS.exports.172.25.80.29@tcp.uuid=c13b8c2a-f6cc-4b1d-a955-05e603f1ed26
mgs.MGS.exports.172.25.80.30@tcp.uuid=ccbde25b-bd34-4601-b3bf-cb8704ed9266
"#;

        let result = parse().easy_parse(x).unwrap();

        assert_debug_snapshot!(result)
    }
}

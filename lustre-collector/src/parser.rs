// Copyright (c) 2021 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    ldlm, llite, mdd_parser,
    mds::{self, client_count_parser},
    mgs::mgs_parser,
    osd_parser, oss, quota, top_level_parser,
    types::Record,
};
use combine::{Parser, Stream, choice, error::ParseError, many};

pub fn params() -> Vec<String> {
    top_level_parser::top_level_params()
        .into_iter()
        .chain(client_count_parser::params())
        .chain(osd_parser::params())
        .chain(mgs_parser::params())
        .chain(oss::params())
        .chain(mds::params())
        .chain(ldlm::params())
        .chain(llite::params())
        .chain(mdd_parser::params())
        .chain(quota::params())
        .collect()
}

pub fn parse<I>() -> impl Parser<I, Output = Vec<Record>>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    many(choice((
        top_level_parser::parse().map(|x| vec![x]),
        client_count_parser::parse(),
        osd_parser::parse().map(|x| vec![x]),
        mgs_parser::parse().map(|x| vec![x]),
        oss::parse().map(|x| vec![x]),
        mds::parse().map(|x| vec![x]),
        ldlm::parse().map(|x| vec![x]),
        llite::parse().map(|x| vec![x]),
        mdd_parser::parse().map(|x| vec![x]),
        quota::parse().map(|x| vec![x]),
    )))
    .map(|xs: Vec<_>| xs.into_iter().flatten().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use combine::parser::EasyParser;
    use include_dir::{Dir, include_dir};
    use insta::assert_debug_snapshot;

    static VALID_FIXTURES: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/src/fixtures/valid/");

    macro_rules! test_fixtures {
        ($name:ident, $pattern:expr) => {
            #[test]
            fn $name() {
                for dir in VALID_FIXTURES.find($pattern).unwrap() {
                    match dir {
                        include_dir::DirEntry::Dir(_) => {}
                        include_dir::DirEntry::File(file) => {
                            let name = file.path().to_string_lossy();

                            let contents = file.contents_utf8().unwrap();

                            let result = parse()
                                .easy_parse(contents)
                                .map_err(|err| err.map_position(|p| p.translate_position(contents)))
                                .unwrap();

                            assert_debug_snapshot!(format!("valid_fixture_{name}"), result);
                        }
                    }
                }
            }
        };
    }

    test_fixtures!(test_valid_fixtures, "*");
    test_fixtures!(test_lustre_2_14_0_ddn145_fixtures, "*ddn145*");

    test_fixtures!(test_lustre_2_14_0_ddn133_fixtures, "*ddn133*");

    #[test]
    fn test_params() {
        assert_debug_snapshot!(params());
    }

    #[test]
    fn test_mdt_output() {
        let x = r#"memused=343719411
memused_max=344830779
lnet_memused=140274209
health_check=healthy
mdt.fs-MDT0000.exports.0@lo.uuid=fs-MDT0000-lwp-MDT0000_UUID
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
ldlm.namespaces.mdt-fs-MDT0000_UUID.contended_locks=32
ldlm.namespaces.mdt-fs2-MDT0000_UUID.contended_locks=32
ldlm.namespaces.mdt-fs-MDT0000_UUID.contention_seconds=2
ldlm.namespaces.mdt-fs2-MDT0000_UUID.contention_seconds=2
ldlm.namespaces.mdt-fs-MDT0000_UUID.ctime_age_limit=10
ldlm.namespaces.mdt-fs2-MDT0000_UUID.ctime_age_limit=10
ldlm.namespaces.mdt-fs-MDT0000_UUID.early_lock_cancel=0
ldlm.namespaces.mdt-fs2-MDT0000_UUID.early_lock_cancel=0
ldlm.namespaces.mdt-fs-MDT0000_UUID.lock_count=0
ldlm.namespaces.mdt-fs2-MDT0000_UUID.lock_count=0
ldlm.namespaces.mdt-fs-MDT0000_UUID.lock_timeouts=0
ldlm.namespaces.mdt-fs2-MDT0000_UUID.lock_timeouts=0
ldlm.namespaces.mdt-fs-MDT0000_UUID.lock_unused_count=0
ldlm.namespaces.mdt-fs2-MDT0000_UUID.lock_unused_count=0
ldlm.namespaces.mdt-fs-MDT0000_UUID.lru_max_age=3900000
ldlm.namespaces.mdt-fs2-MDT0000_UUID.lru_max_age=3900000
ldlm.namespaces.mdt-fs-MDT0000_UUID.lru_size=800
ldlm.namespaces.mdt-fs2-MDT0000_UUID.lru_size=800
ldlm.namespaces.mdt-fs-MDT0000_UUID.max_nolock_bytes=0
ldlm.namespaces.mdt-fs2-MDT0000_UUID.max_nolock_bytes=0
ldlm.namespaces.mdt-fs-MDT0000_UUID.max_parallel_ast=1024
ldlm.namespaces.mdt-fs2-MDT0000_UUID.max_parallel_ast=1024
ldlm.namespaces.mdt-fs-MDT0000_UUID.resource_count=0
ldlm.namespaces.mdt-fs2-MDT0000_UUID.resource_count=0
mdt.fs-MDT0000.md_stats=
snapshot_time             1583789082.568118366 secs.nsecs
getattr                   4 samples [reqs]
statfs                    2 samples [reqs]
mdt.fs2-MDT0000.md_stats=
snapshot_time             1583789082.568222478 secs.nsecs
getattr                   2 samples [reqs]
statfs                    2 samples [reqs]
mdt.fs-MDT0000.num_exports=15
mdt.fs2-MDT0000.num_exports=7
osd-ldiskfs.fs-MDT0000.filesfree=2621151
osd-ldiskfs.fs2-MDT0000.filesfree=2621167
osd-ldiskfs.fs-MDT0000.filestotal=2621440
osd-ldiskfs.fs2-MDT0000.filestotal=2621440
osd-ldiskfs.fs-MDT0000.kbytesavail=1531876
osd-ldiskfs.fs2-MDT0000.kbytesavail=1532068
osd-ldiskfs.fs-MDT0000.kbytesfree=1793268
osd-ldiskfs.fs2-MDT0000.kbytesfree=1793460
osd-ldiskfs.fs-MDT0000.kbytestotal=1819968
osd-ldiskfs.fs2-MDT0000.kbytestotal=1819968
"#;

        let result = parse()
            .easy_parse(x)
            .map_err(|err| err.map_position(|p| p.translate_position(x)))
            .unwrap();

        assert_debug_snapshot!(result);
    }
}

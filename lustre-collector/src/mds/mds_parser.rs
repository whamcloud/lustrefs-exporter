// Copyright (c) 2024 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    MdsStat,
    base_parsers::{equals, period, target},
    stats_parser::stats,
    types::{Param, Record, Stat, Target, TargetStats},
};
use combine::{Parser, attempt, choice, error::ParseError, parser::char::string, stream::Stream};

const STATS: &str = "stats";
const MDS_UPPER: &str = "MDS";

pub(crate) const MDS: &str = "mds";
pub(crate) const MDT: &str = "mdt";
pub(crate) const MDT_FLD: &str = "mdt_fld";
pub(crate) const MDT_IO: &str = "mdt_io";
pub(crate) const MDT_OUT: &str = "mdt_out";
pub(crate) const MDT_READ: &str = "mdt_readpage";
pub(crate) const MDT_SEQM: &str = "mdt_seqm";
pub(crate) const MDT_SEQS: &str = "mdt_seqs";
pub(crate) const MDT_SETATTR: &str = "mdt_setattr";

pub(crate) const MDT_STATS: [&str; 8] = [
    MDT,
    MDT_FLD,
    MDT_IO,
    MDT_OUT,
    MDT_READ,
    MDT_SEQM,
    MDT_SEQS,
    MDT_SETATTR,
];

/// Takes [`MDT_STATS`] and produces a list of params for
/// consumption in proper ltcl get_param format.
pub(crate) fn params() -> Vec<String> {
    MDT_STATS
        .iter()
        .map(|x| format!("{MDS}.{MDS_UPPER}.{x}.{STATS}"))
        .collect()
}

fn mds_prefix<I>() -> impl Parser<I, Output = Target>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (string(MDS).skip(period()), target().skip(period()))
        .map(|(_, x)| x)
        .message("while parsing `mds_prefix`")
}

fn param_non_final<I>(x: &'static str) -> impl Parser<I, Output = Param>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    attempt(string(x).skip(period()))
        .skip(string(STATS).skip(equals()))
        .map(|x| Param(x.to_string()))
        .message("while parsing `mds_suffix`")
}

fn mds_stat<I>() -> impl Parser<I, Output = (Param, Vec<Stat>)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        choice((
            param_non_final(MDT),
            param_non_final(MDT_FLD),
            param_non_final(MDT_IO),
            param_non_final(MDT_OUT),
            param_non_final(MDT_READ),
            param_non_final(MDT_SEQM),
            param_non_final(MDT_SEQS),
            param_non_final(MDT_SETATTR),
        )),
        stats(),
    )
        .message("while parsing `mds_stat`")
}

pub(crate) fn parse<I>() -> impl Parser<I, Output = Record>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    mds_prefix()
        .with(mds_stat())
        .map(|(param, stats)| TargetStats::Mds(MdsStat { param, stats }))
        .map(Record::Target)
        .message("while parsing mds")
}

#[cfg(test)]
mod tests {
    use super::*;
    use combine::{many, parser::EasyParser};
    use insta::assert_debug_snapshot;

    #[test]
    fn test_params() {
        let x = r#"mds.MDS.mdt.stats=
snapshot_time             1689062826.416705941 secs.nsecs
req_waittime              96931 samples [usec] 4 62710 5997491 90147428825
req_qdepth                96931 samples [reqs] 0 2 433 455
req_active                96931 samples [reqs] 1 4 127024 195224
req_timeout               96931 samples [sec] 1 15 1453215 21794505
reqbuf_avail              214247 samples [bufs] 63 64 13711216 877480528
ldlm_ibits_enqueue        14567 samples [reqs] 1 1 14567 14567
mds_reint_setattr         257 samples [reqs] 1 1 257 257
mds_reint_create          2 samples [reqs] 1 1 2 2
mds_reint_open            5505 samples [reqs] 1 1 5505 5505
ost_set_info              3 samples [usec] 11 19 47 771
mds_connect               88 samples [usec] 13 4222 15363 40886015
mds_get_root              1 samples [usec] 5 5 5 25
mds_statfs                4 samples [usec] 14 35 100 2726
mds_sync                  256 samples [usec] 8 45 5212 119940
obd_ping                  81753 samples [usec] 2 63010 2811336 56636492420
mds.MDS.mdt_fld.stats=
snapshot_time             1689062826.416782077 secs.nsecs
req_waittime              65 samples [usec] 6 42 1212 25042
req_qdepth                65 samples [reqs] 0 0 0 0
req_active                65 samples [reqs] 1 1 65 65
req_timeout               65 samples [sec] 1 15 186 1956
reqbuf_avail              141 samples [bufs] 63 64 9012 576012
fld_query                 57 samples [usec] 3 23 510 6280
fld_read                  8 samples [usec] 11 42 220 6736
mds.MDS.mdt_io.stats=snapshot_time             1689062826.416807892 secs.nsecs
mds.MDS.mdt_out.stats=
snapshot_time             1689062826.416820124 secs.nsecs
req_waittime              42447 samples [usec] 12 22802 1589380 2854834950
req_qdepth                42447 samples [reqs] 0 0 0 0
req_active                42447 samples [reqs] 1 2 42451 42459
req_timeout               42447 samples [sec] 15 15 636705 9550575
reqbuf_avail              85306 samples [bufs] 63 64 5458793 349312919
mds_statfs                42437 samples [usec] 5 11264 1188972 162527406
out_update                10 samples [usec] 9 24 146 2296
mds.MDS.mdt_readpage.stats=
snapshot_time             1689062826.416854039 secs.nsecs
req_waittime              5506 samples [usec] 3 641 120123 4566199
req_qdepth                5506 samples [reqs] 0 1 12 12
req_active                5506 samples [reqs] 1 3 6103 7421
req_timeout               5506 samples [sec] 15 15 82590 1238850
reqbuf_avail              11604 samples [bufs] 63 64 740345 47236487
mds_getattr               1 samples [usec] 40 40 40 1600
mds_close                 5505 samples [usec] 11 245 178562 7560868
mds.MDS.mdt_seqm.stats=
snapshot_time             1689062826.416885077 secs.nsecs
req_waittime              1 samples [usec] 28 28 28 784
req_qdepth                1 samples [reqs] 0 0 0 0
req_active                1 samples [reqs] 1 1 1 1
req_timeout               1 samples [sec] 15 15 15 225
reqbuf_avail              3 samples [bufs] 64 64 192 12288
seq_query                 1 samples [usec] 14 14 14 196
mds.MDS.mdt_seqs.stats=
snapshot_time             1689062826.416927653 secs.nsecs
req_waittime              16 samples [usec] 17 3399 7042 21343934
req_qdepth                16 samples [reqs] 0 0 0 0
req_active                16 samples [reqs] 1 3 26 52
req_timeout               16 samples [sec] 1 10 25 115
reqbuf_avail              37 samples [bufs] 63 64 2364 151044
seq_query                 16 samples [usec] 119 3577 17742 46177518
mds.MDS.mdt_setattr.stats=
snapshot_time             1689062826.416952373 secs.nsecs
"#;

        let result: (Vec<_>, _) = many(parse()).easy_parse(x).unwrap();

        assert_debug_snapshot!(result)
    }
}

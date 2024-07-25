// Copyright (c) 2024 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

pub(crate) mod job_stats;
pub(crate) mod obdfilter_parser;
pub(crate) mod oss_parser;

use crate::types::Record;
use combine::{attempt, error::ParseError, Parser, RangeStream};

pub(crate) fn params() -> Vec<String> {
    obdfilter_parser::obd_params()
        .into_iter()
        .chain(oss_parser::params())
        .collect()
}

pub(crate) fn params_no_jobstats() -> Vec<String> {
    obdfilter_parser::obd_params_no_jobstats()
        .into_iter()
        .chain(oss_parser::params())
        .collect()
}

pub(crate) fn params_jobstats_only() -> Vec<String> {
    obdfilter_parser::obd_params_jobstats_only()
}

pub(crate) fn parse<'a, I>() -> impl Parser<I, Output = Record<'a>> + 'a
where
    I: RangeStream<Token = char, Range = &'a str> + 'a,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    attempt(obdfilter_parser::parse()).or(attempt(oss_parser::parse()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use combine::many;
    use insta::assert_debug_snapshot;

    #[test]
    fn test_parse() {
        let x = r#"obdfilter.fs-OST0000.stats=
snapshot_time             1535148988.363769785 secs.nsecs
write_bytes               9 samples [bytes] 98303 4194304 33554431
create                    4 samples [reqs]
statfs                    42297 samples [reqs]
get_info                  2 samples [reqs]
connect                   6 samples [reqs]
reconnect                 1 samples [reqs]
disconnect                4 samples [reqs]
statfs                    46806 samples [reqs]
preprw                    9 samples [reqs]
commitrw                  9 samples [reqs]
ping                      8229 samples [reqs]
obdfilter.fs-OST0000.num_exports=2
obdfilter.fs-OST0000.tot_dirty=0
obdfilter.fs-OST0000.tot_granted=8666816
obdfilter.fs-OST0000.tot_pending=0
ost.OSS.ost.stats=
snapshot_time             1688128253.497763049 secs.nsecs
req_waittime              18419628 samples [usec] 2 40983 305482965 25043535105
req_qdepth                18419628 samples [reqs] 0 34 99937 130635
req_active                18419628 samples [reqs] 1 36 69585063 634492353
req_timeout               18419628 samples [sec] 1 15 276294334 4144414654
reqbuf_avail              38185151 samples [bufs] 60 64 2438170078 155685175822
ldlm_glimpse_enqueue      9257180 samples [reqs] 1 1 9257180 9257180
ldlm_extent_enqueue       19856 samples [reqs] 1 1 19856 19856
ost_create                144904 samples [usec] 6 16594 98795730 85661707326
ost_destroy               8988941 samples [usec] 89 173579 5160119682 8184502010174
ost_get_info              8 samples [usec] 540 3603 10971 28145019
ost_connect               341 samples [usec] 21 903 24182 2818080
ost_disconnect            331 samples [usec] 23 524 39358 7068516
ost_sync                  4510 samples [usec] 3 10945 997271 2117171965
ost_set_info              28 samples [usec] 9 34 606 14594
obd_ping                  3529 samples [usec] 3 12431 60722 155336592
ost.OSS.ost_io.stats=
snapshot_time             1688128269.170769339 secs.nsecs
req_waittime              3398592545 samples [usec] 2 585517 95316362073 32500246129015
req_qdepth                3398592545 samples [reqs] 0 53 90676247 112259319
req_active                3398592545 samples [reqs] 1 82 55496806665 1427593461517
req_timeout               3398592545 samples [sec] 15 15 50978888175 764683322625
reqbuf_avail              7234158916 samples [bufs] 55 64 461878663298 29490702443182
ost_read                  2447557926 samples [usec] 23 138321 1871024223288 4497819893384848
ost_write                 951033049 samples [usec] 59 1247713 2749050524782 100048363896296658
ost_punch                 1515 samples [usec] 16 4883 63967 29511205
"#;

        let result: (Vec<_>, _) = many(parse()).parse(x).unwrap();

        assert_debug_snapshot!(result)
    }
}

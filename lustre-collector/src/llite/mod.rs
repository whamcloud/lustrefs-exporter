// Copyright (c) 2024 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    base_parsers::{param, period, target},
    stats_parser::stats,
    Param, Record, Stat, Target, TargetStats,
};
use combine::{parser::char::string, ParseError, Parser, Stream};

pub(crate) const LLITE: &str = "llite";
pub(crate) const STATS: &str = "stats";

pub(crate) fn params() -> Vec<String> {
    [STATS]
        .into_iter()
        .map(|x| format!("{LLITE}.*.{x}"))
        .collect()
}

fn target_name<I>() -> impl Parser<I, Output = Target>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (string(LLITE).skip(period()), target().skip(period()))
        .map(|(_, x)| x)
        .message("while parsing llite target_name")
}

enum LliteStat {
    Stats(Vec<Stat>),
}

fn llite_stat<I>() -> impl Parser<I, Output = (Param, LliteStat)>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (param(STATS), stats().map(LliteStat::Stats)).message("while parsing llite_stat")
}

pub(crate) fn parse<'a, I>() -> impl Parser<I, Output = Record<'a>>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (target_name(), llite_stat())
        .map(|(target, (param, value))| match value {
            LliteStat::Stats(stats) => TargetStats::Llite(crate::types::LliteStat {
                target,
                param,
                stats,
            }),
        })
        .map(Record::Target)
        .message("while parsing llite")
}

#[cfg(test)]
mod tests {
    use super::*;
    use combine::many;
    use insta::assert_debug_snapshot;

    #[test]
    fn test_parse() {
        let x = r#"llite.ai400x2-ffff9440f1003000.stats=
snapshot_time             1689697369.331040915 secs.nsecs
ioctl                     2 samples [reqs]
open                      13812423 samples [usec] 1 725287 1027077752 8835364169944
close                     13812423 samples [usec] 47 778498 1320315612 17542973849370
readdir                   12 samples [usec] 0 4647 6715 22456295
getattr                   14812440 samples [usec] 2 320411 1317584841 2110166912709
unlink                    6906208 samples [usec] 117 749323 1386719680 23443327087798
mkdir                     7906554 samples [usec] 104 1529199 20996782592 1837945636486522
rmdir                     6939862 samples [usec] 95 646028 16617944601 635123583760591
mknod                     6906208 samples [usec] 119 775827 1454511094 10119157242014
statfs                    7 samples [usec] 147 197 1236 220284
inode_permission          251887103 samples [usec] 0 14235 178199279 1102415701
opencount                 13812424 samples [reqs] 1 2 20718632 34531048
openclosetime             6906208 samples [usec] 2225920 34405427 163169641155255 11416538743473681487
"#;

        let result: (Vec<_>, _) = many(parse()).parse(x).unwrap();

        assert_debug_snapshot!(result)
    }
}

// Copyright (c) 2021 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{types::JobStatOst, BytesStat, ReqsStat, UnsignedLustreTimestamp};
use combine::{
    eof,
    error::{ParseError, StreamError},
    many, optional,
    parser::{
        char::newline,
        range::{range, take_fn, take_until_range, take_while, TakeRange},
    },
    stream::StreamErrorFor,
    Parser, RangeStream,
};

pub(crate) struct JobstatsHeader<'a> {
    pub job_id: &'a str,
    pub snapshot_time: UnsignedLustreTimestamp,
    pub start_time: Option<UnsignedLustreTimestamp>,
    pub elapsed_time: Option<&'a str>,
}

// This function is a rough translation of the following "copy" parser implementation:
// `take_until(attempt((newline(), alpha_num()).map(drop).or(eof())))`
pub(crate) fn find_next_jobstats(haystack: &[u8]) -> Option<usize> {
    // We are looking for newline followed by an alphanumeric char, this indicates a new jobstats entry (from a new OST/MDT)
    // The trick is jobstats entry will always contains a space as it's YAML formatted like below (prefix spaces are replaced with `~`):
    // ```
    // job_stats:
    // - job_id:          cp.0
    // ~~snapshot_time:   1537070542
    // ```
    // So any newline followed by a space is still part of the YAML definition.
    Some(
        memchr::memchr_iter(b'\n', haystack)
            .find(|&i| {
                haystack[i + 1..]
                    .first()
                    .map(|x| x.is_ascii_alphanumeric())
                    .unwrap_or(false)
            })
            // If the newline followed by a space sequence is not found, it indicates there is no more data after the YAML definition so we are at the EOF.
            .unwrap_or(haystack.len()),
    )
}

pub(crate) fn take_jobstats<'a, I>() -> impl Parser<I, Output = &'a str> + 'a
where
    I: RangeStream<Token = char, Range = &'a str> + 'a,
    I::Range: AsRef<[u8]> + combine::stream::Range,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    take_fn(move |haystack: I::Range| {
        let haystack = haystack.as_ref();
        match find_next_jobstats(haystack) {
            Some(i) => TakeRange::Found(i),
            None => TakeRange::NotFound(haystack.len()),
        }
    })
}

pub(crate) fn take_and_skip<'a, I>(input: &'a str) -> impl Parser<I, Output = &'a str> + 'a
where
    I: RangeStream<Token = char, Range = &'a str> + 'a,
    I::Range: AsRef<[u8]> + combine::stream::Range,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    take_until_range(input)
        .skip(range(input))
        .skip(take_while(|c: char| c.is_whitespace()))
}

pub(crate) fn take_bytes_stats<'a, I>() -> impl Parser<I, Output = BytesStat> + 'a
where
    I: RangeStream<Token = char, Range = &'a str> + 'a,
    I::Range: AsRef<[u8]> + combine::stream::Range,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        take_and_skip("{ samples:"),
        take_and_skip(","), // read_bytes_sample
        take_and_skip("unit:"),
        take_until_range(","), // unit
        take_and_skip("min:"),
        take_until_range(","), // min
        take_and_skip("max:"),
        take_until_range(","), // max
        take_and_skip("sum:"),
        take_while(|c: char| c.is_numeric()), // sum
        take_until_range("}"),
    )
        .and_then(
            |(_, sample, _, unit, _, min, _, max, _, sum, _): (
                _,
                &str,
                _,
                &str,
                _,
                &str,
                _,
                &str,
                _,
                &str,
                _,
            )| {
                let samples = sample.parse().map_err(StreamErrorFor::<I>::other)?;
                let min = min.parse().map_err(StreamErrorFor::<I>::other)?;
                let max = max.parse().map_err(StreamErrorFor::<I>::other)?;
                let sum = sum.parse().map_err(StreamErrorFor::<I>::other)?;

                Ok::<BytesStat, StreamErrorFor<I>>(BytesStat {
                    samples,
                    unit: unit.to_string(),
                    min,
                    max,
                    sum,
                })
            },
        )
}

pub(crate) fn take_reqs_stats<'a, I>() -> impl Parser<I, Output = ReqsStat> + 'a
where
    I: RangeStream<Token = char, Range = &'a str> + 'a,
    I::Range: AsRef<[u8]> + combine::stream::Range,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        take_and_skip("{ samples:"),
        take_until_range(","), // sample
        take_and_skip("unit:"),
        take_while(|c: char| c != '}' && c != ','), // unit
    )
        .and_then(|(_, sample, _, unit): (_, &str, _, &str)| {
            let samples = sample.parse().map_err(StreamErrorFor::<I>::other)?;
            Ok::<ReqsStat, StreamErrorFor<I>>(ReqsStat {
                samples,
                unit: unit.trim().to_string(),
            })
        })
}

pub(crate) fn take_jobstats_header<'a, I>() -> impl Parser<I, Output = JobstatsHeader<'a>> + 'a
where
    I: RangeStream<Token = char, Range = &'a str> + 'a,
    I::Range: AsRef<[u8]> + combine::stream::Range,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        take_and_skip("- job_id:").with(take_until_range("\n")),
        take_and_skip("snapshot_time:").with(take_until_range("\n")),
        optional(take_and_skip("start_time:").with(take_until_range("\n"))),
        optional(take_and_skip("elapsed_time:").with(take_until_range("\n"))),
    )
        .and_then(
            |(job_id, snapshot_time, start_time, elapsed_time): (
                &str,
                &str,
                Option<&str>,
                Option<&str>,
            )| {
                // Convert snapshot_time
                let snapshot_time = UnsignedLustreTimestamp::try_from(snapshot_time.to_string())
                    .map_err(StreamErrorFor::<I>::other)?;

                // Convert start_time if it exists
                let start_time = match start_time {
                    Some(time_str) => Some(
                        UnsignedLustreTimestamp::try_from(time_str.to_string())
                            .map_err(StreamErrorFor::<I>::other)?,
                    ),
                    None => None,
                };

                Ok::<JobstatsHeader, StreamErrorFor<I>>(JobstatsHeader {
                    job_id,
                    snapshot_time,
                    start_time,
                    elapsed_time,
                })
            },
        )
}

#[allow(clippy::type_complexity)]
pub(crate) fn take_jobstats_yaml<'a, I>() -> impl Parser<I, Output = Option<Vec<JobStatOst>>> + 'a
where
    I: RangeStream<Token = char, Range = &'a str> + 'a,
    I::Range: AsRef<[u8]> + combine::stream::Range,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    many((
        take_jobstats_header(),
        take_and_skip("read_bytes:").with(take_bytes_stats()),
        take_until_range("write_bytes:").with(take_bytes_stats()),
        take_until_range("getattr:").with(take_reqs_stats()),
        take_until_range("setattr:").with(take_reqs_stats()),
        take_until_range("punch:").with(take_reqs_stats()),
        take_until_range("sync:").with(take_reqs_stats()),
        take_until_range("destroy:").with(take_reqs_stats()),
        take_until_range("create:").with(take_reqs_stats()),
        take_until_range("statfs:").with(take_reqs_stats()),
        take_until_range("get_info:").with(take_reqs_stats()),
        take_until_range("set_info:").with(take_reqs_stats()),
        take_until_range("quotactl:").with(take_reqs_stats()),
        optional(take_until_range("prealloc:").with(take_reqs_stats())),
        optional(take_until_range("}").skip(range("}"))),
    ))
    .map(|x: Vec<_>| {
        let jobstats: Vec<JobStatOst> = x
            .into_iter()
            .map(
                |(
                    jobstats_header,
                    read_bytes,
                    write_bytes,
                    getattr,
                    setattr,
                    punch,
                    sync,
                    destroy,
                    create,
                    statfs,
                    get_info,
                    set_info,
                    quotactl,
                    _,
                    _,
                ): (
                    JobstatsHeader,
                    BytesStat,
                    BytesStat,
                    ReqsStat,
                    ReqsStat,
                    ReqsStat,
                    ReqsStat,
                    ReqsStat,
                    ReqsStat,
                    ReqsStat,
                    ReqsStat,
                    ReqsStat,
                    ReqsStat,
                    _,
                    _,
                )| {
                    JobStatOst {
                        job_id: jobstats_header.job_id.to_string().replace('"', ""),
                        snapshot_time: jobstats_header.snapshot_time,
                        start_time: jobstats_header
                            .start_time,
                        elapsed_time: jobstats_header.elapsed_time.map(|x| x.to_string()),
                        read_bytes,
                        write_bytes,
                        getattr,
                        setattr,
                        punch,
                        sync,
                        destroy,
                        create,
                        statfs,
                        get_info,
                        set_info,
                        quotactl,
                    }
                },
            )
            .collect();

        if jobstats.is_empty() {
            None
        } else {
            Some(jobstats)
        }
    })
}

pub(crate) fn parse<'a, I>() -> impl Parser<I, Output = Option<Vec<JobStatOst>>> + 'a
where
    I: RangeStream<Token = char, Range = &'a str> + 'a,
    I::Range: AsRef<[u8]> + combine::stream::Range,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        optional(newline()), // If Jobstats are present, the whole yaml blob will be on a newline
        take_jobstats(),
    )
        .skip(optional(newline()))
        .skip(optional(eof()))
        .and_then(|(_, yaml_blob): (_, &str)| {
            if yaml_blob.trim().is_empty() {
                return Ok(None);
            }
            take_jobstats_yaml()
                .parse(yaml_blob)
                .map(|(x, _)| x)
                .map_err(StreamErrorFor::<I>::other)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        types::{BytesStat, ReqsStat},
        JobStatsOst, UnsignedLustreTimestamp,
    };

    #[test]
    fn test_yaml_deserialize() {
        let x = r#"job_stats:
- job_id:          cp.0
  snapshot_time:   1537070542
  read_bytes:      { samples:         256, unit: bytes, min: 4194304, max: 4194304, sum:      1073741824 }
  write_bytes:     { samples:           0, unit: bytes, min:       0, max:       0, sum:               0 }
  getattr:         { samples:           0, unit:  reqs }
  setattr:         { samples:           0, unit:  reqs }
  punch:           { samples:           0, unit:  reqs }
  sync:            { samples:           0, unit:  reqs }
  destroy:         { samples:           0, unit:  reqs }
  create:          { samples:           0, unit:  reqs }
  statfs:          { samples:           0, unit:  reqs }
  get_info:        { samples:           0, unit:  reqs }
  set_info:        { samples:           0, unit:  reqs }
  quotactl:        { samples:           0, unit:  reqs }"#;

        let expected = JobStatsOst {
            job_stats: Some(vec![JobStatOst {
                job_id: "cp.0".to_string(),
                snapshot_time: UnsignedLustreTimestamp(1_537_070_542),
                start_time: None,
                elapsed_time: None,
                read_bytes: BytesStat {
                    samples: 256,
                    unit: "bytes".to_string(),
                    min: 4_194_304,
                    max: 4_194_304,
                    sum: 1_073_741_824,
                },
                write_bytes: BytesStat {
                    samples: 0,
                    unit: "bytes".to_string(),
                    min: 0,
                    max: 0,
                    sum: 0,
                },
                getattr: ReqsStat {
                    samples: 0,
                    unit: "reqs".to_string(),
                },
                setattr: ReqsStat {
                    samples: 0,
                    unit: "reqs".to_string(),
                },
                punch: ReqsStat {
                    samples: 0,
                    unit: "reqs".to_string(),
                },
                sync: ReqsStat {
                    samples: 0,
                    unit: "reqs".to_string(),
                },
                destroy: ReqsStat {
                    samples: 0,
                    unit: "reqs".to_string(),
                },
                create: ReqsStat {
                    samples: 0,
                    unit: "reqs".to_string(),
                },
                statfs: ReqsStat {
                    samples: 0,
                    unit: "reqs".to_string(),
                },
                get_info: ReqsStat {
                    samples: 0,
                    unit: "reqs".to_string(),
                },
                set_info: ReqsStat {
                    samples: 0,
                    unit: "reqs".to_string(),
                },
                quotactl: ReqsStat {
                    samples: 0,
                    unit: "reqs".to_string(),
                },
            }]),
        };

        assert_eq!(serde_yaml::from_str::<JobStatsOst>(x).unwrap(), expected)
    }

    #[test]
    fn with_time_triple() {
        let x = r#"job_stats:
- job_id:          lfs.0.
  snapshot_time:   1686153914.643122579 secs.nsecs
  start_time:      1686153914.643119181 secs.nsecs
  elapsed_time:    0.000003398 secs.nsecs
  read_bytes:      { samples:           0, unit: bytes, min:        0, max:        0, sum:                0, sumsq:                  0, hist: {  } }
  write_bytes:     { samples:           0, unit: bytes, min:        0, max:        0, sum:                0, sumsq:                  0, hist: {  } }
  read:            { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  write:           { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  getattr:         { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  setattr:         { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  punch:           { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  sync:            { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  destroy:         { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  create:          { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  statfs:          { samples:           1, unit: usecs, min:        1, max:        1, sum:                1, sumsq:                  1 }
  get_info:        { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  set_info:        { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  quotactl:        { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  prealloc:        { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }"#;

        insta::assert_debug_snapshot!(serde_yaml::from_str::<JobStatsOst>(x).unwrap());
    }

    #[test]
    fn test_yaml_combine() {
        let x = r#"- job_id:          "SLURM_JOB_machine176_2453:0:mac"
  snapshot_time:   1721048514.718714674 secs.nsecs
  start_time:      1720764336.685487209 secs.nsecs
  elapsed_time:    284178.033227465 secs.nsecs
  read_bytes:      { samples:           0, unit: bytes, min:        0, max:        0, sum:                0, sumsq:                  0, hist: {  } }
  write_bytes:     { samples:       12999, unit: bytes, min:     4096, max:  1048576, sum:       3918708736, sumsq:   1989431670079488, hist: { 4K: 35, 8K: 49, 16K: 96, 32K: 276, 64K: 837, 128K: 2390, 256K: 3891, 512K: 3380, 1M: 2045 } }
  read:            { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  write:           { samples:       12999, unit: usecs, min:        8, max:  1586470, sum:        748382254, sumsq:    144307310384400 }
  getattr:         { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  setattr:         { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  punch:           { samples:         121, unit: usecs, min:       52, max:    13739, sum:            30462, sumsq:          234586860 }
  sync:            { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  destroy:         { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  create:          { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  statfs:          { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  get_info:        { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  set_info:        { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  quotactl:        { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  prealloc:        { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }"#;

        let res = take_jobstats_yaml().parse(x);
        insta::assert_debug_snapshot!(res);
    }

    #[test]
    fn test_old_yaml_combine() {
        let x = r#"- job_id:          cp.0
  snapshot_time:   1537070542
  read_bytes:      { samples:         256, unit: bytes, min: 4194304, max: 4194304, sum:      1073741824 }
  write_bytes:     { samples:           0, unit: bytes, min:       0, max:       0, sum:               0 }
  getattr:         { samples:           0, unit:  reqs }
  setattr:         { samples:           0, unit:  reqs }
  punch:           { samples:           0, unit:  reqs }
  sync:            { samples:           0, unit:  reqs }
  destroy:         { samples:           0, unit:  reqs }
  create:          { samples:           0, unit:  reqs }
  statfs:          { samples:           0, unit:  reqs }
  get_info:        { samples:           0, unit:  reqs }
  set_info:        { samples:           0, unit:  reqs }
  quotactl:        { samples:           0, unit:  reqs }"#;

        let res = take_jobstats_yaml().parse(x);
        insta::assert_debug_snapshot!(res);
    }
}

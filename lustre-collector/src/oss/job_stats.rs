// Copyright (c) 2021 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::types::{JobStatOst, JobStatsOst};
use combine::{
    eof,
    error::{ParseError, StreamError},
    optional,
    parser::{
        char::newline,
        range::{take_fn, TakeRange},
    },
    stream::StreamErrorFor,
    Parser, RangeStream,
};

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
        .and_then(|(_, x): (_, &str)| {
            serde_yaml::from_str(x)
                .map(|x: JobStatsOst| x.job_stats)
                .map_err(StreamErrorFor::<I>::other)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        types::{BytesStat, ReqsStat},
        UnsignedLustreTimestamp,
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
}

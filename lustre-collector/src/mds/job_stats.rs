// Copyright (c) 2021 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::types::{JobStatMdt, JobStatsMdt};
use combine::{
    attempt,
    error::{ParseError, StreamError},
    optional,
    parser::{
        char::{alpha_num, newline},
        repeat::take_until,
    },
    stream::{Stream, StreamErrorFor},
    Parser,
};

pub(crate) fn parse<I>() -> impl Parser<I, Output = Option<Vec<JobStatMdt>>>
where
    I: Stream<Token = char>,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    (
        optional(newline()), // If Jobstats are present, the whole yaml blob will be on a newline
        take_until(attempt((newline(), alpha_num()))),
    )
        .skip(newline())
        .and_then(|(_, x): (_, String)| {
            serde_yaml::from_str(&x)
                .map(|x: JobStatsMdt| x.job_stats)
                .map_err(StreamErrorFor::<I>::other)
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{types::BytesStat, UnsignedLustreTimestamp};

    #[test]
    fn test_yaml_deserialize() {
        let x = r#"job_stats:
- job_id:          touch.0
  snapshot_time:   1614767417
  open:            { samples:           1, unit: usecs, min:     315, max:     315, sum:             315, sumsq:              99225 }
  close:           { samples:           1, unit: usecs, min:      19, max:      19, sum:              19, sumsq:                361 }
  mknod:           { samples:           1, unit: usecs, min:     296, max:     296, sum:             296, sumsq:              87616 }
  link:            { samples:           0, unit: usecs, min:       0, max:       0, sum:               0, sumsq:                  0 }
  unlink:          { samples:           0, unit: usecs, min:       0, max:       0, sum:               0, sumsq:                  0 }
  mkdir:           { samples:           0, unit: usecs, min:       0, max:       0, sum:               0, sumsq:                  0 }
  rmdir:           { samples:           0, unit: usecs, min:       0, max:       0, sum:               0, sumsq:                  0 }
  rename:          { samples:           0, unit: usecs, min:       0, max:       0, sum:               0, sumsq:                  0 }
  getattr:         { samples:           0, unit: usecs, min:       0, max:       0, sum:               0, sumsq:                  0 }
  setattr:         { samples:           1, unit: usecs, min:      27, max:      27, sum:              27, sumsq:                729 }
  getxattr:        { samples:           0, unit: usecs, min:       0, max:       0, sum:               0, sumsq:                  0 }
  setxattr:        { samples:           0, unit: usecs, min:       0, max:       0, sum:               0, sumsq:                  0 }
  statfs:          { samples:           0, unit: usecs, min:       0, max:       0, sum:               0, sumsq:                  0 }
  sync:            { samples:           0, unit: usecs, min:       0, max:       0, sum:               0, sumsq:                  0 }
  samedir_rename:  { samples:           0, unit: usecs, min:       0, max:       0, sum:               0, sumsq:                  0 }
  crossdir_rename: { samples:           0, unit: usecs, min:       0, max:       0, sum:               0, sumsq:                  0 }
  read_bytes:      { samples:           0, unit: bytes, min:       0, max:       0, sum:               0, sumsq:                  0 }
  write_bytes:     { samples:           0, unit: bytes, min:       0, max:       0, sum:               0, sumsq:                  0 }
  punch:           { samples:           0, unit: usecs, min:       0, max:       0, sum:               0, sumsq:                  0 }"#;

        let expected = JobStatsMdt {
            job_stats: Some(vec![JobStatMdt {
                job_id: "touch.0".to_string(),
                snapshot_time: UnsignedLustreTimestamp(1_614_767_417),
                start_time: None,
                elapsed_time: None,
                open: BytesStat {
                    samples: 1,
                    unit: "usecs".to_string(),
                    min: 315,
                    max: 315,
                    sum: 315,
                },
                close: BytesStat {
                    samples: 1,
                    unit: "usecs".to_string(),
                    min: 19,
                    max: 19,
                    sum: 19,
                },
                mknod: BytesStat {
                    samples: 1,
                    unit: "usecs".to_string(),
                    min: 296,
                    max: 296,
                    sum: 296,
                },
                link: BytesStat {
                    samples: 0,
                    unit: "usecs".to_string(),
                    min: 0,
                    max: 0,
                    sum: 0,
                },
                unlink: BytesStat {
                    samples: 0,
                    unit: "usecs".to_string(),
                    min: 0,
                    max: 0,
                    sum: 0,
                },
                mkdir: BytesStat {
                    samples: 0,
                    unit: "usecs".to_string(),
                    min: 0,
                    max: 0,
                    sum: 0,
                },
                rmdir: BytesStat {
                    samples: 0,
                    unit: "usecs".to_string(),
                    min: 0,
                    max: 0,
                    sum: 0,
                },
                rename: BytesStat {
                    samples: 0,
                    unit: "usecs".to_string(),
                    min: 0,
                    max: 0,
                    sum: 0,
                },
                getattr: BytesStat {
                    samples: 0,
                    unit: "usecs".to_string(),
                    min: 0,
                    max: 0,
                    sum: 0,
                },
                setattr: BytesStat {
                    samples: 1,
                    unit: "usecs".to_string(),
                    min: 27,
                    max: 27,
                    sum: 27,
                },
                getxattr: BytesStat {
                    samples: 0,
                    unit: "usecs".to_string(),
                    min: 0,
                    max: 0,
                    sum: 0,
                },
                setxattr: BytesStat {
                    samples: 0,
                    unit: "usecs".to_string(),
                    min: 0,
                    max: 0,
                    sum: 0,
                },
                statfs: BytesStat {
                    samples: 0,
                    unit: "usecs".to_string(),
                    min: 0,
                    max: 0,
                    sum: 0,
                },
                sync: BytesStat {
                    samples: 0,
                    unit: "usecs".to_string(),
                    min: 0,
                    max: 0,
                    sum: 0,
                },
                samedir_rename: BytesStat {
                    samples: 0,
                    unit: "usecs".to_string(),
                    min: 0,
                    max: 0,
                    sum: 0,
                },
                crossdir_rename: BytesStat {
                    samples: 0,
                    unit: "usecs".to_string(),
                    min: 0,
                    max: 0,
                    sum: 0,
                },
                read_bytes: BytesStat {
                    samples: 0,
                    unit: "bytes".to_string(),
                    min: 0,
                    max: 0,
                    sum: 0,
                },
                write_bytes: BytesStat {
                    samples: 0,
                    unit: "bytes".to_string(),
                    min: 0,
                    max: 0,
                    sum: 0,
                },
                punch: BytesStat {
                    samples: 0,
                    unit: "usecs".to_string(),
                    min: 0,
                    max: 0,
                    sum: 0,
                },
                parallel_rename_dir: None,
                parallel_rename_file: None,
            }]),
        };

        assert_eq!(serde_yaml::from_str::<JobStatsMdt>(x).unwrap(), expected)
    }
}

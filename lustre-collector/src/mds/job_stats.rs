// Copyright (c) 2021 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    oss::job_stats::{
        take_and_skip, take_bytes_stats, take_jobstats, take_jobstats_header, JobstatsHeader,
    },
    types::JobStatMdt,
    BytesStat,
};
use combine::{
    eof,
    error::{ParseError, StreamError},
    many, optional,
    parser::{
        char::newline,
        range::{range, take_until_range},
    },
    stream::StreamErrorFor,
    Parser, RangeStream,
};

pub(crate) fn parse<'a, I>() -> impl Parser<I, Output = Option<Vec<JobStatMdt<'a>>>> + 'a
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

#[allow(clippy::type_complexity)]
pub(crate) fn take_jobstats_yaml<'a, I>(
) -> impl Parser<I, Output = Option<Vec<JobStatMdt<'a>>>> + 'a
where
    I: RangeStream<Token = char, Range = &'a str> + 'a,
    I::Range: AsRef<[u8]> + combine::stream::Range,
    I::Error: ParseError<I::Token, I::Range, I::Position>,
{
    many((
        take_jobstats_header(),
        (
            take_and_skip("open:").with(take_bytes_stats()),
            take_until_range("close:").with(take_bytes_stats()),
            take_until_range("mknod:").with(take_bytes_stats()),
            take_until_range("link:").with(take_bytes_stats()),
            take_until_range("unlink:").with(take_bytes_stats()),
        ),
        (
            take_until_range("mkdir:").with(take_bytes_stats()),
            take_until_range("rmdir:").with(take_bytes_stats()),
            take_until_range("rename:").with(take_bytes_stats()),
            take_until_range("getattr:").with(take_bytes_stats()),
            take_until_range("setattr:").with(take_bytes_stats()),
            take_until_range("getxattr:").with(take_bytes_stats()),
            take_until_range("setxattr:").with(take_bytes_stats()),
            take_until_range("statfs:").with(take_bytes_stats()),
            take_until_range("sync:").with(take_bytes_stats()),
            take_until_range("samedir_rename:").with(take_bytes_stats()),
        ),
        (
            optional(take_until_range("parallel_rename_file:").with(take_bytes_stats())),
            optional(take_until_range("parallel_rename_dir:").with(take_bytes_stats())),
        ),
        take_until_range("crossdir_rename:").with(take_bytes_stats()),
        (
            optional(take_until_range("read:").with(take_bytes_stats())),
            optional(take_until_range("write:").with(take_bytes_stats())),
        ),
        (
            take_until_range("read_bytes:").with(take_bytes_stats()),
            take_until_range("write_bytes:").with(take_bytes_stats()),
            take_until_range("punch:").with(take_bytes_stats()),
        ),
        optional(take_until_range("migrate:").with(take_bytes_stats())),
        take_until_range("}").skip(range("}")),
    ))
    .map(|x: Vec<_>| {
        let jobstats: Vec<JobStatMdt> = x
            .into_iter()
            .map(
                |(
                    jobstats_header,
                    (open, close, mknod, link, unlink),
                    (
                        mkdir,
                        rmdir,
                        rename,
                        getattr,
                        setattr,
                        getxattr,
                        setxattr,
                        statfs,
                        sync,
                        samedir_rename,
                    ),
                    (parallel_rename_file, parallel_rename_dir),
                    crossdir_rename,
                    (_read, _write),
                    (read_bytes, write_bytes, punch),
                    _migrate,
                    _,
                ): (
                    JobstatsHeader,
                    (BytesStat, BytesStat, BytesStat, BytesStat, BytesStat),
                    (
                        BytesStat,
                        BytesStat,
                        BytesStat,
                        BytesStat,
                        BytesStat,
                        BytesStat,
                        BytesStat,
                        BytesStat,
                        BytesStat,
                        BytesStat,
                    ),
                    (Option<BytesStat>, Option<BytesStat>),
                    BytesStat,
                    (Option<BytesStat>, Option<BytesStat>),
                    (BytesStat, BytesStat, BytesStat),
                    Option<BytesStat>,
                    _,
                )| {
                    JobStatMdt {
                        job_id: jobstats_header.job_id,
                        snapshot_time: jobstats_header.snapshot_time,
                        start_time: jobstats_header.start_time,
                        elapsed_time: jobstats_header.elapsed_time,
                        open,
                        close,
                        mknod,
                        link,
                        unlink,
                        mkdir,
                        rmdir,
                        rename,
                        getattr,
                        setattr,
                        getxattr,
                        setxattr,
                        statfs,
                        sync,
                        samedir_rename,
                        crossdir_rename,
                        read_bytes,
                        write_bytes,
                        punch,
                        parallel_rename_dir,
                        parallel_rename_file,
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

#[cfg(test)]
mod tests {
    use insta::assert_debug_snapshot;

    use super::*;
    use crate::{types::BytesStat, JobStatsMdt, UnsignedLustreTimestamp};

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
                job_id: "touch.0",
                snapshot_time: UnsignedLustreTimestamp(1_614_767_417),
                start_time: None,
                elapsed_time: None,
                open: BytesStat {
                    samples: 1,
                    unit: "usecs",
                    min: 315,
                    max: 315,
                    sum: 315,
                },
                close: BytesStat {
                    samples: 1,
                    unit: "usecs",
                    min: 19,
                    max: 19,
                    sum: 19,
                },
                mknod: BytesStat {
                    samples: 1,
                    unit: "usecs",
                    min: 296,
                    max: 296,
                    sum: 296,
                },
                link: BytesStat {
                    samples: 0,
                    unit: "usecs",
                    min: 0,
                    max: 0,
                    sum: 0,
                },
                unlink: BytesStat {
                    samples: 0,
                    unit: "usecs",
                    min: 0,
                    max: 0,
                    sum: 0,
                },
                mkdir: BytesStat {
                    samples: 0,
                    unit: "usecs",
                    min: 0,
                    max: 0,
                    sum: 0,
                },
                rmdir: BytesStat {
                    samples: 0,
                    unit: "usecs",
                    min: 0,
                    max: 0,
                    sum: 0,
                },
                rename: BytesStat {
                    samples: 0,
                    unit: "usecs",
                    min: 0,
                    max: 0,
                    sum: 0,
                },
                getattr: BytesStat {
                    samples: 0,
                    unit: "usecs",
                    min: 0,
                    max: 0,
                    sum: 0,
                },
                setattr: BytesStat {
                    samples: 1,
                    unit: "usecs",
                    min: 27,
                    max: 27,
                    sum: 27,
                },
                getxattr: BytesStat {
                    samples: 0,
                    unit: "usecs",
                    min: 0,
                    max: 0,
                    sum: 0,
                },
                setxattr: BytesStat {
                    samples: 0,
                    unit: "usecs",
                    min: 0,
                    max: 0,
                    sum: 0,
                },
                statfs: BytesStat {
                    samples: 0,
                    unit: "usecs",
                    min: 0,
                    max: 0,
                    sum: 0,
                },
                sync: BytesStat {
                    samples: 0,
                    unit: "usecs",
                    min: 0,
                    max: 0,
                    sum: 0,
                },
                samedir_rename: BytesStat {
                    samples: 0,
                    unit: "usecs",
                    min: 0,
                    max: 0,
                    sum: 0,
                },
                crossdir_rename: BytesStat {
                    samples: 0,
                    unit: "usecs",
                    min: 0,
                    max: 0,
                    sum: 0,
                },
                read_bytes: BytesStat {
                    samples: 0,
                    unit: "bytes",
                    min: 0,
                    max: 0,
                    sum: 0,
                },
                write_bytes: BytesStat {
                    samples: 0,
                    unit: "bytes",
                    min: 0,
                    max: 0,
                    sum: 0,
                },
                punch: BytesStat {
                    samples: 0,
                    unit: "usecs",
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

    #[test]
    fn test_yaml_combine() {
        let x = r#"- job_id:          ":988:ds88"
  snapshot_time:   1721048534.873002069 secs.nsecs
  start_time:      1720767442.368619499 secs.nsecs
  elapsed_time:    281092.504382570 secs.nsecs
  open:            { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  close:           { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  mknod:           { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  link:            { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  unlink:          { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  mkdir:           { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  rmdir:           { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  rename:          { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  getattr:         { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  setattr:         { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  getxattr:        { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  setxattr:        { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  statfs:          { samples:        9372, unit: usecs, min:        8, max:       53, sum:           148887, sumsq:            2456573 }
  sync:            { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  samedir_rename:  { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  parallel_rename_file: { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  parallel_rename_dir: { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  crossdir_rename: { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  read:            { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  write:           { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  read_bytes:      { samples:           0, unit: bytes, min:        0, max:        0, sum:                0, sumsq:                  0, hist: {  } }
  write_bytes:     { samples:           0, unit: bytes, min:        0, max:        0, sum:                0, sumsq:                  0, hist: {  } }
  punch:           { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }
  migrate:         { samples:           0, unit: usecs, min:        0, max:        0, sum:                0, sumsq:                  0 }"#;

        let res = take_jobstats_yaml().parse(x);
        assert_debug_snapshot!(res);
    }
}

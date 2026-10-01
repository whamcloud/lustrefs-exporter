// Copyright (c) 2026 Google LLC
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

//! `lctl get_param` output split into one block per parameter as it is read,
//! so a scrape never holds more than one block of text.

use lustre_collector::{LustreCollectorError, Record};
use std::io::{self, BufRead};

#[derive(Debug, thiserror::Error)]
pub enum BlockError {
    /// The rest of the output is still read.
    #[error("Failed to parse lctl block `{header}`: {source}")]
    Parse {
        header: String,
        source: LustreCollectorError,
    },
    /// Ends the iteration: nothing after the error is available.
    #[error(transparent)]
    Read(#[from] io::Error),
}

/// No value line in any fixture both starts with an alphanumeric character and
/// contains `=`.
fn is_param_header(line: &[u8]) -> bool {
    line.first().is_some_and(u8::is_ascii_alphanumeric) && line.contains(&b'=')
}

/// The collector parses a target's exports as one unit.
fn is_exports_header(line: &[u8]) -> bool {
    contains(line, b".exports.") && contains(line, b".uuid=")
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

fn first_line(block: &[u8]) -> &[u8] {
    block
        .iter()
        .position(|&b| b == b'\n')
        .map_or(block, |end| &block[..end])
}

pub struct LctlRecords<R> {
    reader: R,
    line: Vec<u8>,
    block: Vec<u8>,
    pending: std::vec::IntoIter<Record>,
    done: bool,
}

pub fn lctl_records<R: BufRead>(f: R) -> LctlRecords<R> {
    LctlRecords {
        reader: f,
        line: Vec::new(),
        block: Vec::new(),
        pending: Vec::new().into_iter(),
        done: false,
    }
}

fn parse_block(block: &[u8]) -> Result<Vec<Record>, BlockError> {
    lustre_collector::parse_lctl_block(block).map_err(|source| BlockError::Parse {
        header: String::from_utf8_lossy(first_line(block)).into_owned(),
        source,
    })
}

impl<R: BufRead> LctlRecords<R> {
    fn finish_block(&mut self) -> Result<(), BlockError> {
        let parsed = parse_block(&self.block);
        self.block.clear();
        self.pending = parsed?.into_iter();

        Ok(())
    }
}

impl<R: BufRead> Iterator for LctlRecords<R> {
    type Item = Result<Record, BlockError>;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(record) = self.pending.next() {
                return Some(Ok(record));
            }

            if self.done {
                return None;
            }

            self.line.clear();

            match self.reader.read_until(b'\n', &mut self.line) {
                Ok(0) => {
                    self.done = true;

                    if self.block.is_empty() {
                        return None;
                    }

                    if let Err(e) = self.finish_block() {
                        return Some(Err(e));
                    }

                    continue;
                }
                Ok(_) => {}
                Err(e) => {
                    self.done = true;

                    return Some(Err(e.into()));
                }
            }

            let starts_block = is_param_header(&self.line)
                && !(is_exports_header(&self.line) && is_exports_header(first_line(&self.block)));

            if starts_block && !self.block.is_empty() {
                let finished = self.finish_block();

                self.block.extend_from_slice(&self.line);

                if let Err(e) = finished {
                    return Some(Err(e));
                }
            } else {
                self.block.extend_from_slice(&self.line);
            }

            if !self.line.ends_with(b"\n") {
                self.block.push(b'\n');
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{BufReader, Cursor, Read},
        path::{Path, PathBuf},
    };

    fn fixtures(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in std::fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();

            if path.is_dir() {
                fixtures(&path, out);
            } else if path.extension().is_some_and(|x| x == "txt") {
                out.push(path);
            }
        }
    }

    #[test]
    fn lctl_records_matches_whole_buffer_parse() {
        let dir =
            Path::new(env!("CARGO_MANIFEST_DIR")).join("../lustre-collector/src/fixtures/valid");

        let mut paths = vec![];
        fixtures(&dir, &mut paths);
        paths.sort();

        let mut checked = 0;

        for path in paths {
            let bytes = std::fs::read(&path).unwrap();

            let expected = lustre_collector::parse_lctl_output(&bytes)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));

            let streamed: Vec<Record> = lctl_records(BufReader::new(Cursor::new(bytes)))
                .map(|x| x.unwrap_or_else(|e| panic!("{}: {e}", path.display())))
                .collect();

            assert_eq!(streamed, expected, "{}", path.display());

            checked += 1;
        }

        assert!(checked > 0, "no fixtures found under {}", dir.display());
    }

    #[test]
    fn unparseable_block_is_reported_and_skipped() {
        let input = "memused=1\nnot_a_param=\nsome value\nlnet_memused=2\n";

        let xs: Vec<_> = lctl_records(BufReader::new(Cursor::new(input))).collect();

        assert_eq!(xs.len(), 3);
        assert!(xs[0].is_ok());
        let Err(BlockError::Parse { header, .. }) = &xs[1] else {
            panic!("expected a parse error, got {:?}", xs[1]);
        };
        assert_eq!(header, "not_a_param=");
        assert!(xs[2].is_ok());
    }

    #[test]
    fn invalid_utf8_is_a_parse_error_for_its_block() {
        let input = b"memused=1\nlnet_memused=2\xff\nhealth_check=healthy\n";

        let xs: Vec<_> = lctl_records(BufReader::new(Cursor::new(&input[..]))).collect();

        assert_eq!(xs.len(), 3);
        assert!(xs[0].is_ok());
        assert!(matches!(xs[1], Err(BlockError::Parse { .. })));
        assert!(xs[2].is_ok(), "{:?}", xs[2]);
    }

    struct Failing<'a> {
        data: &'a [u8],
        ok: usize,
    }

    impl Read for Failing<'_> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.ok == 0 {
                return Err(io::Error::new(io::ErrorKind::BrokenPipe, "lctl died"));
            }

            let n = buf.len().min(self.ok).min(self.data.len());
            buf[..n].copy_from_slice(&self.data[..n]);
            self.data = &self.data[n..];
            self.ok -= n;

            Ok(n)
        }
    }

    #[test]
    fn read_error_is_terminal() {
        let input = b"memused=1\nlnet_memused=2\nhealth_check=healthy\n";
        let reader = BufReader::with_capacity(
            8,
            Failing {
                data: input,
                ok: 26,
            },
        );

        let xs: Vec<_> = lctl_records(reader).collect();

        assert_eq!(xs.len(), 2, "{xs:?}");
        assert!(xs[0].is_ok());
        assert!(matches!(xs[1], Err(BlockError::Read(_))), "{:?}", xs[1]);
    }

    #[test]
    fn exports_headers_stay_together() {
        let input = "mdt.fs-MDT0000.exports.0@lo.uuid=\nfs-MDT0000-lwp-MDT0000_UUID\nmdt.fs-MDT0000.exports.10.0.0.1@tcp.uuid=\nabc-def_UUID\nmemused=1\n";

        let expected = lustre_collector::parse_lctl_output(input.as_bytes()).unwrap();

        let streamed: Vec<Record> = lctl_records(BufReader::new(Cursor::new(input)))
            .map(|x| x.unwrap())
            .collect();

        assert_eq!(streamed, expected);
    }
}

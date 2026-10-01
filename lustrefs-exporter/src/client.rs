// Copyright (c) 2026 Google LLC
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

//! Client series carry `fs` next to `target`: a client has one osc/mdc per
//! server target of every filesystem it mounts. [`ClientLabels::ByFilesystem`]
//! drops `target` and sums the series per filesystem.

use crate::{Family, LabelContainer};
use lustre_collector::StatsHeader;
use prometheus_client::metrics::{counter::Counter, gauge::Gauge};
use std::sync::atomic::AtomicU64;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ClientLabels {
    /// One series per target, labelled `fs` and `target`.
    #[default]
    PerTarget,
    /// One series per filesystem, labelled `fs`, summed over its targets.
    ByFilesystem,
}

impl ClientLabels {
    pub fn labels(self, target: &str) -> LabelContainer {
        let mut xs = vec![("fs", fs_name(target).to_string())];

        if self == ClientLabels::PerTarget {
            xs.push(("target", target.to_string()));
        }

        xs
    }
}

/// `server_name2fsname()`: the part before the last `-` or `:` within the
/// first `LUSTRE_MAXFSNAME + 1` characters, so a hyphenated name stays whole.
pub fn fs_name(target: &str) -> &str {
    const LUSTRE_MAXFSNAME: usize = 8;

    let bytes = target.as_bytes();
    let mut i = bytes.len().min(LUSTRE_MAXFSNAME);

    while i > 0 && bytes.get(i).is_none_or(|&b| b != b'-' && b != b':') {
        i -= 1;
    }

    if i == 0 { target } else { &target[..i] }
}

pub fn labels(target: &str) -> LabelContainer {
    ClientLabels::PerTarget.labels(target)
}

fn merge(
    family: &Family<Gauge<u64, AtomicU64>>,
    labels: &LabelContainer,
    value: u64,
    pick: fn(u64, u64) -> u64,
) {
    if let Some(gauge) = family.get(labels) {
        gauge.set(pick(gauge.get(), value));

        return;
    }

    family.get_or_create(labels).set(value);
}

/// The smallest value the series has been given in this scrape.
pub fn merge_min(family: &Family<Gauge<u64, AtomicU64>>, labels: &LabelContainer, value: u64) {
    merge(family, labels, value, u64::min);
}

/// The largest value the series has been given in this scrape.
pub fn merge_max(family: &Family<Gauge<u64, AtomicU64>>, labels: &LabelContainer, value: u64) {
    merge(family, labels, value, u64::max);
}

pub fn with(labels: &LabelContainer, name: &'static str, value: String) -> LabelContainer {
    let mut xs = labels.clone();
    xs.push((name, value));
    xs
}

/// One gauge per block, not per counter (GCP-226); a summed series keeps
/// the earliest start.
pub fn set_start_time(
    family: &Family<Gauge<u64, AtomicU64>>,
    labels: &LabelContainer,
    header: &StatsHeader,
) {
    let start = header
        .start_time
        .as_ref()
        .and_then(|s| s.parse::<f64>().ok())
        .map(|f| f as u64);

    if let Some(start) = start {
        merge_min(family, labels, start);
    }
}

/// One counter per bucket and direction, as `lustre_disk_io_total` renders
/// `brw_stats`; each item is `(size, read, write)`.
pub fn observe_rw(
    family: &Family<Counter<u64>>,
    labels: &LabelContainer,
    buckets: impl IntoIterator<Item = (u64, u64, u64)>,
) {
    for (size, read, write) in buckets {
        let size = size.to_string();

        for (operation, count) in [("read", read), ("write", write)] {
            let mut xs = with(labels, "operation", operation.to_string());
            xs.push(("size", size.clone()));
            family.get_or_create(&xs).inc_by(count);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn labels_by_mode() {
        let target = "a361000-OST0000-osc-ffff949bc0626000";

        assert_eq!(
            ClientLabels::PerTarget.labels(target),
            vec![
                ("fs", "a361000".to_string()),
                ("target", target.to_string())
            ]
        );
        assert_eq!(
            ClientLabels::ByFilesystem.labels(target),
            vec![("fs", "a361000".to_string())]
        );
    }

    #[test]
    fn merged_gauges_keep_the_extremes() {
        let min = Family::default();
        let max = Family::default();
        let labels = vec![("fs", "a361000".to_string())];

        for value in [5, 0, 3] {
            merge_min(&min, &labels, value);
            merge_max(&max, &labels, value);
        }

        assert_eq!(min.get_or_create(&labels).get(), 0);
        assert_eq!(max.get_or_create(&labels).get(), 5);
    }

    #[test]
    fn fs_name_follows_the_kernel_rule() {
        assert_eq!(fs_name("a361000-OST0000-osc-ffff949bc0626000"), "a361000");
        assert_eq!(fs_name("a361000-ffff949bc0626000"), "a361000");
        assert_eq!(fs_name("lustre-MDT0000-mdc-ffff949bc0626000"), "lustre");
        assert_eq!(fs_name("my-fs-OST0000-osc-ffff949bc0626000"), "my-fs");
        assert_eq!(fs_name("abcdefgh-OST0000-osc-ffff949bc0626000"), "abcdefgh");
        assert_eq!(fs_name("lustre:OST0000-osc-ffff949bc0626000"), "lustre");
        assert_eq!(fs_name("nodash"), "nodash");
    }
}

// Copyright (c) 2026 Google LLC
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

//! Bucketed client tables as one counter per bucket or as histograms.
//!
//! `lprocfs_oh_tally_log2()` files a value under `fls(value - 1)`, so bucket
//! `i` holds `(2^(i-1), 2^i]` and bucket 31 holds everything larger. Each
//! table prints its own key for bucket `i`: pages per RPC prints `2^i`, the
//! upper bound; offset and the latencies print `2^(i-1)`, the lower edge;
//! RPCs in flight is tallied linearly and prints `i`. Offset is tallied as
//! `offset + 1`, so its bound is `2k - 1`. The kernel keeps no sum, so
//! `_sum` is count times upper bound.

use crate::{
    Family, LabelContainer,
    client::{observe_rw, with},
};
use prometheus_client::{
    encoding::{EncodeMetric, MetricEncoder, NoLabelSet},
    metrics::{MetricType, TypedMetric, counter::Counter},
    registry::Registry,
};
use std::{
    collections::BTreeMap,
    iter,
    sync::{PoisonError, RwLock},
};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum HistogramEncoding {
    #[default]
    BucketCounters,
    Histogram,
}

/// `OBD_HIST_MAX - 1`: the kernel clamps larger values into this bucket.
const OVERFLOW_INDEX: u32 = 31;

/// A kernel bucket: the key it prints and the inclusive upper bound it
/// stands for, `None` for the overflow bucket (`+Inf`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Bucket {
    pub key: u64,
    pub le: Option<u64>,
}

/// Which bound a table's printed key is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Table {
    /// log2; the key is the upper bound.
    PagesPerRpc,
    /// linear; the key is the value.
    RpcsInFlight,
    /// log2 of `offset + 1`; the key is the lower edge.
    Offset,
    /// log2 of binary microseconds (ns >> 10); the key is the lower edge.
    Latency,
}

impl Table {
    pub fn bucket(self, key: u64) -> Bucket {
        let le = match self {
            Self::PagesPerRpc => Some(key),
            Self::RpcsInFlight if key == u64::from(OVERFLOW_INDEX) => None,
            Self::RpcsInFlight => Some(key),
            Self::Offset | Self::Latency if key == 1 << (OVERFLOW_INDEX - 1) => None,
            Self::Offset => Some((2 * key).saturating_sub(1)),
            Self::Latency => Some((2 * key).max(1)),
        };

        Bucket { key, le }
    }
}

/// Per-bucket counts by upper bound; the encoder accumulates them.
#[derive(Debug, Default)]
pub struct BucketHistogram {
    inner: RwLock<Inner>,
}

#[derive(Debug, Default)]
struct Inner {
    sum: f64,
    buckets: BTreeMap<u64, u64>,
    overflow: u64,
}

impl BucketHistogram {
    pub fn observe(&self, bucket: Bucket, count: u64) {
        let mut inner = self.inner.write().unwrap_or_else(PoisonError::into_inner);

        match bucket.le {
            Some(le) => *inner.buckets.entry(le).or_default() += count,
            None => inner.overflow += count,
        }
        inner.sum += count as f64 * bucket.le.unwrap_or(bucket.key) as f64;
    }
}

impl EncodeMetric for BucketHistogram {
    fn encode(&self, mut encoder: MetricEncoder) -> Result<(), std::fmt::Error> {
        let inner = self.inner.read().unwrap_or_else(PoisonError::into_inner);

        let buckets: Vec<(f64, u64)> = inner
            .buckets
            .iter()
            .map(|(le, count)| (*le as f64, *count))
            .chain(iter::once((f64::MAX, inner.overflow)))
            .collect();
        let count = buckets.iter().map(|(_, count)| count).sum();

        encoder.encode_histogram::<NoLabelSet>(inner.sum, count, &buckets, None)
    }

    fn metric_type(&self) -> MetricType {
        Self::TYPE
    }
}

impl TypedMetric for BucketHistogram {
    const TYPE: MetricType = MetricType::Histogram;
}

#[derive(Debug)]
pub enum Bucketed {
    Counters(Family<Counter<u64>>),
    Histograms(Family<BucketHistogram>),
}

impl Default for Bucketed {
    fn default() -> Self {
        Self::new(HistogramEncoding::default())
    }
}

impl Bucketed {
    pub fn new(encoding: HistogramEncoding) -> Self {
        match encoding {
            HistogramEncoding::BucketCounters => Self::Counters(Family::default()),
            HistogramEncoding::Histogram => Self::Histograms(Family::default()),
        }
    }

    /// The counters count events and keep the unit in their `size` label;
    /// the histogram observes the value, so it is named `<name>_<unit>`.
    pub fn register(
        &self,
        registry: &mut Registry,
        name: &str,
        unit: Option<&str>,
        help: &str,
        size: &str,
        bounds: &str,
    ) {
        match self {
            Self::Counters(family) => registry.register(
                name,
                format!("{help}. 'size' label is {size}"),
                family.clone(),
            ),
            Self::Histograms(family) => registry.register(
                unit.map_or_else(|| name.to_string(), |unit| format!("{name}_{unit}")),
                format!(
                    "{help}. Bucket bounds are {bounds}; _sum is count times bucket upper bound, not observed"
                ),
                family.clone(),
            ),
        }
    }

    pub fn observe(
        &self,
        labels: &LabelContainer,
        buckets: impl IntoIterator<Item = (Bucket, u64)>,
    ) {
        match self {
            Self::Counters(family) => {
                for (bucket, count) in buckets {
                    family
                        .get_or_create(&with(labels, "size", bucket.key.to_string()))
                        .inc_by(count);
                }
            }
            Self::Histograms(family) => {
                let histogram = family.get_or_create(labels);

                for (bucket, count) in buckets {
                    histogram.observe(bucket, count);
                }
            }
        }
    }

    /// Each item is `(bucket, read, write)`.
    pub fn observe_rw(
        &self,
        labels: &LabelContainer,
        buckets: impl IntoIterator<Item = (Bucket, u64, u64)>,
    ) {
        match self {
            Self::Counters(family) => observe_rw(
                family,
                labels,
                buckets
                    .into_iter()
                    .map(|(bucket, read, write)| (bucket.key, read, write)),
            ),
            Self::Histograms(family) => {
                let read = with(labels, "operation", "read".to_string());
                let write = with(labels, "operation", "write".to_string());

                for (bucket, r, w) in buckets {
                    family.get_or_create(&read).observe(bucket, r);
                    family.get_or_create(&write).observe(bucket, w);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use prometheus_client::encoding::text::encode;

    fn encode_buckets(buckets: impl IntoIterator<Item = (Option<u64>, u64)>) -> Vec<String> {
        let family: Family<BucketHistogram> = Family::default();
        let histogram = family.get_or_create(&vec![("fs", "a".to_string())]);

        for (le, count) in buckets {
            histogram.observe(Bucket { key: 1 << 30, le }, count);
        }
        drop(histogram);

        let mut registry = Registry::default();
        registry.register("h", "help", family);

        let mut out = String::new();
        encode(&mut out, &registry).unwrap();

        out.lines()
            .filter(|l| l.starts_with("h_"))
            .map(str::to_string)
            .collect()
    }

    #[test]
    fn buckets_are_cumulative_on_the_wire_and_end_with_inf() {
        assert_eq!(
            encode_buckets([(Some(1), 5), (Some(4), 0), (Some(2), 3), (Some(2), 1)]),
            [
                "h_sum{fs=\"a\"} 13.0",
                "h_count{fs=\"a\"} 9",
                "h_bucket{le=\"1.0\",fs=\"a\"} 5",
                "h_bucket{le=\"2.0\",fs=\"a\"} 9",
                "h_bucket{le=\"4.0\",fs=\"a\"} 9",
                "h_bucket{le=\"+Inf\",fs=\"a\"} 9",
            ]
        );
    }

    #[test]
    fn overflow_bucket_lands_in_inf() {
        assert_eq!(
            encode_buckets([(Some(8), 2), (None, 3)]),
            [
                "h_sum{fs=\"a\"} 3221225488.0",
                "h_count{fs=\"a\"} 5",
                "h_bucket{le=\"8.0\",fs=\"a\"} 2",
                "h_bucket{le=\"+Inf\",fs=\"a\"} 5",
            ]
        );
    }

    #[test]
    fn table_bounds() {
        let le = |t: Table, k| t.bucket(k).le;

        assert_eq!(le(Table::PagesPerRpc, 1), Some(1));
        assert_eq!(le(Table::PagesPerRpc, 256), Some(256));

        assert_eq!(le(Table::RpcsInFlight, 0), Some(0));
        assert_eq!(le(Table::RpcsInFlight, 30), Some(30));
        assert_eq!(le(Table::RpcsInFlight, 31), None);

        assert_eq!(le(Table::Offset, 0), Some(0));
        assert_eq!(le(Table::Offset, 1), Some(1));
        assert_eq!(le(Table::Offset, 8), Some(15));
        assert_eq!(le(Table::Offset, 1 << 30), None);

        assert_eq!(le(Table::Latency, 0), Some(1));
        assert_eq!(le(Table::Latency, 512), Some(1024));
        assert_eq!(le(Table::Latency, 1 << 30), None);
    }
}

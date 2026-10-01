// Copyright (c) 2026 Google LLC
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

//! `osc.<target>.*` and `mdc.<target>.*`: the same families for both,
//! named `lustre_client_<osc|mdc>_*`, plus the ones only one side prints.

use crate::{
    Family, LabelProm,
    client::{ClientLabels, ExtendedClientMetrics, merge_max, merge_min, set_start_time, with},
    histogram::{Bucketed, HistogramEncoding, Table},
};
use lustre_collector::{
    BrwStats, ControllerStat, ControllerVariant, KeyValue, LocklessStats, RpcStats, Stat,
    TimedControllerStat,
};
use prometheus_client::{
    metrics::{counter::Counter, gauge::Gauge},
    registry::Registry,
};
use std::sync::atomic::AtomicU64;

#[derive(Debug)]
pub struct ClientTargetMetrics {
    component: ControllerVariant,
    labels: ClientLabels,
    extended: ExtendedClientMetrics,
    stats_total: Family<Counter<u64>>,
    stats_start_time: Family<Gauge<u64, AtomicU64>>,
    stats_time_min: Family<Gauge<u64, AtomicU64>>,
    stats_time_max: Family<Gauge<u64, AtomicU64>>,
    stats_time_sum: Family<Counter<u64>>,
    rpc_read_in_flight: Family<Gauge<u64, AtomicU64>>,
    rpc_write_in_flight: Family<Gauge<u64, AtomicU64>>,
    rpc_pending_write_pages: Family<Gauge<u64, AtomicU64>>,
    rpc_pending_read_pages: Family<Gauge<u64, AtomicU64>>,
    rpc_pages_per_rpc_total: Bucketed,
    rpc_bytes_per_rpc_total: Bucketed,
    rpc_rpcs_in_flight_total: Bucketed,
    rpc_offset_total: Bucketed,
    lockless_read_bytes_total: Family<Counter<u64>>,
    lockless_write_bytes_total: Family<Counter<u64>>,
    lockless_truncates_total: Family<Counter<u64>>,
    // osc only
    read_bytes_total: Family<Counter<u64>>,
    write_bytes_total: Family<Counter<u64>>,
    rpc_dio_in_flight: Family<Gauge<u64, AtomicU64>>,
    rpc_latency_total: Bucketed,
    io_latency_total: Bucketed,
    unstable_pages: Family<Gauge<u64, AtomicU64>>,
    compress_write_pages: Family<Counter<u64>>,
    compress_write_chunks: Family<Counter<u64>>,
    compress_write_bytes: Family<Counter<u64>>,
    compress_read_pages: Family<Counter<u64>>,
    compress_read_chunks: Family<Counter<u64>>,
    compress_read_bytes: Family<Counter<u64>>,
    cur_grant_bytes: Family<Gauge<u64, AtomicU64>>,
    cur_dirty_bytes: Family<Gauge<u64, AtomicU64>>,
    // mdc only
    md_stats_requests_total: Family<Counter<u64>>,
    rpc_modify_in_flight: Family<Gauge<u64, AtomicU64>>,
    rpc_modify_rpcs_in_flight_total: Bucketed,
}

impl ClientTargetMetrics {
    pub fn new(
        component: ControllerVariant,
        labels: ClientLabels,
        histograms: HistogramEncoding,
        extended: ExtendedClientMetrics,
    ) -> Self {
        Self {
            component,
            labels,
            extended,
            stats_total: Family::default(),
            stats_start_time: Family::default(),
            stats_time_min: Family::default(),
            stats_time_max: Family::default(),
            stats_time_sum: Family::default(),
            rpc_read_in_flight: Family::default(),
            rpc_write_in_flight: Family::default(),
            rpc_pending_write_pages: Family::default(),
            rpc_pending_read_pages: Family::default(),
            rpc_pages_per_rpc_total: Bucketed::new(histograms),
            rpc_bytes_per_rpc_total: Bucketed::new(histograms),
            rpc_rpcs_in_flight_total: Bucketed::new(histograms),
            rpc_offset_total: Bucketed::new(histograms),
            lockless_read_bytes_total: Family::default(),
            lockless_write_bytes_total: Family::default(),
            lockless_truncates_total: Family::default(),
            read_bytes_total: Family::default(),
            write_bytes_total: Family::default(),
            rpc_dio_in_flight: Family::default(),
            rpc_latency_total: Bucketed::new(histograms),
            io_latency_total: Bucketed::new(histograms),
            unstable_pages: Family::default(),
            compress_write_pages: Family::default(),
            compress_write_chunks: Family::default(),
            compress_write_bytes: Family::default(),
            compress_read_pages: Family::default(),
            compress_read_chunks: Family::default(),
            compress_read_bytes: Family::default(),
            cur_grant_bytes: Family::default(),
            cur_dirty_bytes: Family::default(),
            md_stats_requests_total: Family::default(),
            rpc_modify_in_flight: Family::default(),
            rpc_modify_rpcs_in_flight_total: Bucketed::new(histograms),
        }
    }

    pub fn register_metric(&self, registry: &mut Registry) {
        let c = self.component.to_prom_label();
        let name = |suffix: &str| format!("lustre_client_{c}_{suffix}");

        if self.component == ControllerVariant::Mdc {
            registry.register(
                name("md_stats_requests"),
                "Total number of metadata requests, by operation",
                self.md_stats_requests_total.clone(),
            );
        }
        registry.register(
            name("stats"),
            "Number of RPCs the client has sent to the target, by operation",
            self.stats_total.clone(),
        );
        registry.register(
            name("stats_start_time"),
            match self.labels {
                ClientLabels::PerTarget => "Unix epoch seconds when the stats were last reset",
                ClientLabels::ByFilesystem => {
                    "Unix epoch seconds of the earliest stats reset among the filesystem's targets"
                }
            },
            self.stats_start_time.clone(),
        );
        registry.register(
            name("stats_time_microseconds_min"),
            "Minimum time taken for an operation, by operation, in microseconds",
            self.stats_time_min.clone(),
        );
        registry.register(
            name("stats_time_microseconds_max"),
            "Maximum time taken for an operation, by operation, in microseconds",
            self.stats_time_max.clone(),
        );
        registry.register(
            name("stats_time_microseconds"),
            "Total time taken by operations, by operation, in microseconds",
            self.stats_time_sum.clone(),
        );
        if self.component == ControllerVariant::Osc {
            registry.register(
                name("read_bytes"),
                "Total bytes read by the client via OSC",
                self.read_bytes_total.clone(),
            );
            registry.register(
                name("write_bytes"),
                "Total bytes written by the client via OSC",
                self.write_bytes_total.clone(),
            );
        }
        if self.component == ControllerVariant::Mdc {
            registry.register(
                name("rpc_stats_modify_in_flight"),
                "Number of modify RPCs in flight",
                self.rpc_modify_in_flight.clone(),
            );
        }
        registry.register(
            name("rpc_stats_read_in_flight"),
            "Number of read RPCs in flight",
            self.rpc_read_in_flight.clone(),
        );
        registry.register(
            name("rpc_stats_write_in_flight"),
            "Number of write RPCs in flight",
            self.rpc_write_in_flight.clone(),
        );
        if self.component == ControllerVariant::Osc {
            registry.register(
                name("rpc_stats_dio_in_flight"),
                "Number of direct I/O RPCs in flight (DDN 2.14 builds, upstream 2.17.51+)",
                self.rpc_dio_in_flight.clone(),
            );
        }
        registry.register(
            name("rpc_stats_pending_write_pages"),
            "Number of pages waiting to be written",
            self.rpc_pending_write_pages.clone(),
        );
        registry.register(
            name("rpc_stats_pending_read_pages"),
            "Number of pages waiting to be read",
            self.rpc_pending_read_pages.clone(),
        );
        if self.component == ControllerVariant::Mdc {
            self.rpc_modify_rpcs_in_flight_total.register(
                registry,
                &name("rpc_stats_modify_rpcs_in_flight"),
                None,
                "Number of modify RPCs by modify RPCs in flight when issued",
                "the number in flight",
                "modify RPCs in flight, exact values",
            );
        }
        self.rpc_pages_per_rpc_total.register(
            registry,
            &name("rpc_stats_pages_per_rpc"),
            None,
            "Number of RPCs by pages per RPC",
            "the inclusive upper edge of the pages-per-RPC bucket",
            "pages per RPC, inclusive upper bounds",
        );
        if self.extended != ExtendedClientMetrics::Off {
            self.rpc_bytes_per_rpc_total.register(
                registry,
                &name("rpc_stats_bytes_per_rpc"),
                None,
                "Number of RPCs by bytes per RPC: pages per RPC times this kernel's page size",
                "the inclusive upper edge of the bucket in bytes",
                "bytes per RPC, inclusive upper bounds",
            );
        }
        self.rpc_rpcs_in_flight_total.register(
            registry,
            &name("rpc_stats_rpcs_in_flight"),
            None,
            "Number of RPCs by RPCs in flight when issued",
            "the number of RPCs in flight",
            "RPCs in flight, exact values",
        );
        self.rpc_offset_total.register(
            registry,
            &name("rpc_stats_offset"),
            None,
            "Number of RPCs by starting file offset",
            "the kernel's log2 bucket key, the lower edge of the bucket in pages",
            "starting offset in pages, inclusive upper bounds",
        );
        if self.component == ControllerVariant::Osc {
            self.rpc_latency_total.register(
                registry,
                &name("rpc_stats_latency"),
                Some("microseconds"),
                "Number of RPCs (a count, not a time) by round-trip latency (DDN 2.14 builds, upstream 2.17.51+)",
                "the lower edge of the latency bucket in microseconds",
                "latency in binary microseconds (1024 ns units), inclusive upper bounds",
            );
            self.io_latency_total.register(
                registry,
                &name("io_latency"),
                Some("microseconds"),
                "Number of I/Os (a count, not a time) by latency and I/O size (DDN 2.14 builds, upstream 2.17.51+)",
                "the lower edge of the latency bucket in microseconds, 'opsize' the lower bound of the I/O size bucket",
                "latency in binary microseconds (1024 ns units), inclusive upper bounds; 'opsize' is the lower bound of the I/O size bucket",
            );
        }
        registry.register(
            name("lockless_read_bytes"),
            "Total lockless read bytes",
            self.lockless_read_bytes_total.clone(),
        );
        registry.register(
            name("lockless_write_bytes"),
            "Total lockless write bytes",
            self.lockless_write_bytes_total.clone(),
        );
        registry.register(
            name("lockless_truncates"),
            "Total lockless truncates (printed by Lustre 2.12 to 2.14.52 only)",
            self.lockless_truncates_total.clone(),
        );
        if self.component == ControllerVariant::Osc {
            registry.register(
                name("unstable_pages"),
                "Number of unstable pages",
                self.unstable_pages.clone(),
            );
            for (suffix, help, family) in [
                (
                    "compress_write_pages",
                    "write page",
                    &self.compress_write_pages,
                ),
                (
                    "compress_write_chunks",
                    "write chunk",
                    &self.compress_write_chunks,
                ),
                (
                    "compress_write_bytes",
                    "write byte",
                    &self.compress_write_bytes,
                ),
                (
                    "compress_read_pages",
                    "read page",
                    &self.compress_read_pages,
                ),
                (
                    "compress_read_chunks",
                    "read chunk",
                    &self.compress_read_chunks,
                ),
                (
                    "compress_read_bytes",
                    "read byte",
                    &self.compress_read_bytes,
                ),
            ] {
                registry.register(
                    name(suffix),
                    format!(
                        "RPC compression {help} counters (DDN builds). 'type' label is the key suffix as the kernel prints it"
                    ),
                    family.clone(),
                );
            }
            registry.register(
                name("cur_grant_bytes"),
                "Bytes of write grant the client currently holds from the OST",
                self.cur_grant_bytes.clone(),
            );
            registry.register(
                name("cur_dirty_bytes"),
                "Bytes of dirty data the client currently caches for the OST",
                self.cur_dirty_bytes.clone(),
            );
        }
    }
}

pub fn build_md_stats(x: &TimedControllerStat<Vec<Stat>>, metrics: &mut ClientTargetMetrics) {
    let labels = metrics.labels.labels(&x.controller);

    for stat in &x.value {
        metrics
            .md_stats_requests_total
            .get_or_create(&with(&labels, "operation", stat.name.clone()))
            .inc_by(stat.samples);
    }
}

/// `read_bytes` and `write_bytes` also feed the byte counters.
pub fn build_stats(x: &TimedControllerStat<Vec<Stat>>, metrics: &mut ClientTargetMetrics) {
    let labels = metrics.labels.labels(&x.controller);

    set_start_time(&metrics.stats_start_time, &labels, &x.header);

    for stat in &x.value {
        let op = with(&labels, "operation", stat.name.clone());

        metrics.stats_total.get_or_create(&op).inc_by(stat.samples);

        if stat.units.starts_with("usec") {
            if let Some(min) = stat.min {
                merge_min(&metrics.stats_time_min, &op, min);
            }
            if let Some(max) = stat.max {
                merge_max(&metrics.stats_time_max, &op, max);
            }
            if let Some(sum) = stat.sum {
                metrics.stats_time_sum.get_or_create(&op).inc_by(sum);
            }
        }

        let bytes = match stat.name.as_str() {
            "read_bytes" => &metrics.read_bytes_total,
            "write_bytes" => &metrics.write_bytes_total,
            _ => continue,
        };

        if let Some(sum) = stat.sum {
            bytes.get_or_create(&labels).inc_by(sum);
        }
    }
}

pub fn build_rpc_stats(x: &TimedControllerStat<RpcStats>, metrics: &mut ClientTargetMetrics) {
    let labels = metrics.labels.labels(&x.controller);

    for scalar in &x.value.scalars {
        let family = match scalar.name.as_str() {
            "modify_RPCs_in_flight" => &metrics.rpc_modify_in_flight,
            "read RPCs in flight" => &metrics.rpc_read_in_flight,
            "write RPCs in flight" => &metrics.rpc_write_in_flight,
            "DIO RPCs in flight" => &metrics.rpc_dio_in_flight,
            "pending write pages" => &metrics.rpc_pending_write_pages,
            "pending read pages" => &metrics.rpc_pending_read_pages,
            other => {
                tracing::warn!("{}: unknown rpc_stats value {other:?}", &*x.controller);
                continue;
            }
        };

        family.get_or_create(&labels).inc_by(scalar.value);
    }

    if let Some(buckets) = &x.value.modify_rpcs_in_flight {
        metrics.rpc_modify_rpcs_in_flight_total.observe(
            &labels,
            buckets
                .iter()
                .map(|b| (Table::RpcsInFlight.bucket(b.key), b.count)),
        );
    }

    for histogram in &x.value.histograms {
        let (family, table) = match histogram.name.as_str() {
            "pages per rpc" => (&metrics.rpc_pages_per_rpc_total, Table::PagesPerRpc),
            "rpcs in flight" => (&metrics.rpc_rpcs_in_flight_total, Table::RpcsInFlight),
            "offset" => (&metrics.rpc_offset_total, Table::Offset),
            "RPC latency (us)" => (&metrics.rpc_latency_total, Table::Latency),
            other => {
                tracing::warn!("{}: unknown rpc_stats table {other:?}", &*x.controller);
                continue;
            }
        };

        family.observe_rw(
            &labels,
            histogram
                .buckets
                .iter()
                .map(|b| (table.bucket(b.name), b.read, b.write)),
        );

        if let (Table::PagesPerRpc, ExtendedClientMetrics::On { page_size }) =
            (table, metrics.extended)
        {
            metrics.rpc_bytes_per_rpc_total.observe_rw(
                &labels,
                histogram
                    .buckets
                    .iter()
                    .map(|b| (table.bucket(b.name).scaled(page_size), b.read, b.write)),
            );
        }
    }
}

/// The collector groups the kernel's `rd_<size>` and `wr_<size>` lines into
/// one `io_time_<size>` entry.
pub fn build_io_latency_stats(
    x: &TimedControllerStat<Vec<BrwStats>>,
    metrics: &mut ClientTargetMetrics,
) {
    let labels = metrics.labels.labels(&x.controller);

    for BrwStats { name, buckets, .. } in &x.value {
        let Some(opsize) = name.strip_prefix("io_time_") else {
            tracing::warn!(
                "{}: unknown io_latency_stats entry {name:?}",
                &*x.controller
            );
            continue;
        };

        metrics.io_latency_total.observe_rw(
            &with(&labels, "opsize", opsize.to_string()),
            buckets
                .iter()
                .map(|b| (Table::Latency.bucket(b.name), b.read, b.write)),
        );
    }
}

pub fn build_lockless_stats(
    x: &TimedControllerStat<LocklessStats>,
    metrics: &mut ClientTargetMetrics,
) {
    let labels = metrics.labels.labels(&x.controller);

    metrics
        .lockless_read_bytes_total
        .get_or_create(&labels)
        .inc_by(x.value.read_bytes);
    metrics
        .lockless_write_bytes_total
        .get_or_create(&labels)
        .inc_by(x.value.write_bytes);
    if let Some(truncates) = x.value.truncates {
        metrics
            .lockless_truncates_total
            .get_or_create(&labels)
            .inc_by(truncates);
    }
}

pub fn build_unstable_stats(x: &ControllerStat<Vec<KeyValue>>, metrics: &mut ClientTargetMetrics) {
    let labels = metrics.labels.labels(&x.controller);

    for kv in &x.value {
        let family = match kv.name.as_str() {
            "unstable_pages" => &metrics.unstable_pages,
            // unstable_mb is derived from unstable_pages.
            "unstable_mb" => continue,
            other => {
                tracing::warn!("{}: unknown unstable_stats value {other:?}", &*x.controller);
                continue;
            }
        };

        family.get_or_create(&labels).inc_by(kv.value);
    }
}

/// DDN builds. Keys are `<read|write>_<pages|chunks|bytes>_<type>`.
pub fn build_compression_stats(
    x: &ControllerStat<Vec<KeyValue>>,
    metrics: &mut ClientTargetMetrics,
) {
    let labels = metrics.labels.labels(&x.controller);

    for kv in &x.value {
        let (family, kind) = match kv.name.split('_').collect::<Vec<_>>().as_slice() {
            ["write", "pages", kind] => (&metrics.compress_write_pages, *kind),
            ["write", "chunks", kind] => (&metrics.compress_write_chunks, *kind),
            ["write", "bytes", kind] => (&metrics.compress_write_bytes, *kind),
            ["read", "pages", kind] => (&metrics.compress_read_pages, *kind),
            ["read", "chunks", kind] => (&metrics.compress_read_chunks, *kind),
            ["read", "bytes", kind] => (&metrics.compress_read_bytes, *kind),
            _ => {
                tracing::warn!(
                    "{}: unknown stats_compr value {:?}",
                    &*x.controller,
                    kv.name
                );
                continue;
            }
        };

        family
            .get_or_create(&with(&labels, "type", kind.to_string()))
            .inc_by(kv.value);
    }
}

pub fn build_cur_grant_bytes(x: &ControllerStat<u64>, metrics: &mut ClientTargetMetrics) {
    metrics
        .cur_grant_bytes
        .get_or_create(&metrics.labels.labels(&x.controller))
        .inc_by(x.value);
}

pub fn build_cur_dirty_bytes(x: &ControllerStat<u64>, metrics: &mut ClientTargetMetrics) {
    metrics
        .cur_dirty_bytes
        .get_or_create(&metrics.labels.labels(&x.controller))
        .inc_by(x.value);
}

// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    Error, Family,
    brw_stats::{BrwStatsMetrics, build_target_stats},
    client::{ClientLabels, ExtendedClientMetrics},
    controller::{ControllerMetrics, build_controller_stats},
    histogram::HistogramEncoding,
    host::{HostMetrics, build_host_stats},
    llite::LliteMetrics,
    lnet::{LNetMetrics, build_lnet_stats},
    quota::QuotaMetrics,
    service::{ServiceMetrics, build_service_stats},
    stats::StatsMetrics,
    stream::BlockError,
};
use lustre_collector::Record;
use prometheus_client::{metrics::gauge::Gauge, registry::Registry};
use std::{collections::HashSet, sync::atomic::AtomicU64};

pub type TargetSet = HashSet<(String, String, String, String)>;

#[derive(Debug, Default)]
pub struct Metrics {
    pub host: HostMetrics,
    pub quota: QuotaMetrics,
    pub service: ServiceMetrics,
    pub brw: BrwStatsMetrics,
    pub controller: ControllerMetrics,
    pub llite: LliteMetrics,
    pub lnet: LNetMetrics,
    pub stats: StatsMetrics,
    pub export: StatsMetrics,
    pub mds: StatsMetrics, // Reusing the Stats structure for MDS metrics
    target_info: Family<Gauge<u64, AtomicU64>>,
    // Gauges: Metrics is rebuilt for every scrape.
    parse_errors: Family<Gauge<u64, AtomicU64>>,
    command_errors: Family<Gauge<u64, AtomicU64>>,
}

impl Metrics {
    pub fn new(
        client_labels: ClientLabels,
        histograms: HistogramEncoding,
        extended: ExtendedClientMetrics,
    ) -> Self {
        Self {
            controller: ControllerMetrics::new(client_labels, histograms, extended),
            llite: LliteMetrics::new(histograms),
            ..Self::default()
        }
    }

    pub fn register_metric(&self, registry: &mut Registry) {
        self.host.register_metric(registry);
        self.quota.register_metric(registry);
        self.service.register_metric(registry);
        self.brw.register_metric(registry);
        self.controller.register_metric(registry);
        self.llite.register_metric(registry);
        self.lnet.register_metric(registry);
        self.stats.register_metric(registry);
        self.export.register_metric(registry);
        self.mds.register_metric(registry);

        // prometheus_client does not automatically include the `target_info` metric.
        // Add it manually.
        registry.register("target_info", "Target metadata", self.target_info.clone());

        registry.register(
            "lustre_exporter_parse_errors",
            "Number of blocks of command output the exporter could not parse in this scrape; those blocks are missing from it",
            self.parse_errors.clone(),
        );
        registry.register(
            "lustre_exporter_command_errors",
            "Number of error lines a command printed on stderr in this scrape, not counting patterns that match nothing on this node; what it could not read is missing from the scrape",
            self.command_errors.clone(),
        );
    }

    pub fn record_command_errors(&self, command: &'static str, count: u64, first: &str) {
        tracing::warn!("{command} printed {count} error line(s); first: {first}");

        self.command_errors
            .get_or_create(&vec![("command", command.to_string())])
            .set(count);
    }

    pub fn record_parse_errors(&self, source: &'static str, count: u64, first: &str) {
        tracing::warn!(
            "{count} block(s) of {source} output could not be parsed and were skipped; first: `{first}` (details at debug)"
        );

        self.parse_errors
            .get_or_create(&vec![("source", source.to_string())])
            .set(count);
    }
}

pub fn process_record(x: &Record, metrics: &mut Metrics, set: &mut TargetSet) {
    match x {
        lustre_collector::Record::Host(x) => {
            build_host_stats(x, &mut metrics.host);
        }
        lustre_collector::Record::LNetStat(x) => {
            build_lnet_stats(x, &mut metrics.lnet);
        }
        lustre_collector::Record::Target(x) => {
            build_target_stats(x, metrics, set);
        }
        lustre_collector::Record::Controller(x) => {
            build_controller_stats(x, metrics);
        }
        lustre_collector::Record::LustreService(x) => {
            build_service_stats(x, &mut metrics.service);
        }
        _ => {}
    }
}

pub fn build_lustre_stats(output: &Vec<Record>, metrics: &mut Metrics) {
    // This set is used to store the possible duplicate target stats
    let mut set = HashSet::new();

    for x in output {
        process_record(x, metrics, &mut set);
    }
}

/// A block that fails to parse is counted and logged; a read error fails the
/// scrape, since the rest of the output is gone.
pub fn fold_records(
    records: impl IntoIterator<Item = Result<Record, BlockError>>,
    source: &'static str,
    metrics: &mut Metrics,
    set: &mut TargetSet,
) -> Result<(), Error> {
    let mut failed = 0u64;
    let mut first = None;

    for x in records {
        match x {
            Ok(record) => process_record(&record, metrics, set),
            Err(BlockError::Read(e)) => return Err(e.into()),
            Err(BlockError::Parse { header, source: e }) => {
                tracing::debug!("Failed to parse {source} block `{header}`: {e}");

                if first.is_none() {
                    first = Some(header);
                }

                failed += 1;
            }
        }
    }

    if let Some(first) = first {
        metrics.record_parse_errors(source, failed, &first);
    }

    Ok(())
}

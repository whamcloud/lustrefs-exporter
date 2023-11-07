use std::{collections::BTreeMap, ops::Deref};

use crate::{LabelProm, Metric, StatsMapExt};
use lustre_collector::{Stat, Target, TargetStat};
use prometheus_exporter_base::prelude::*;

static READ_SAMPLES: Metric = Metric {
    name: "lustre_read_samples_total",
    help: "Total number of reads that have been recorded.",
    r#type: MetricType::Counter,
};
static READ_MIN_SIZE_BYTES: Metric = Metric {
    name: "lustre_read_minimum_size_bytes",
    help: "The minimum read size in bytes.",
    r#type: MetricType::Gauge,
};
static READ_MAX_SIZE_BYTES: Metric = Metric {
    name: "lustre_read_maximum_size_bytes",
    help: "The maximum read size in bytes.",
    r#type: MetricType::Gauge,
};
static READ_BYTES: Metric = Metric {
    name: "lustre_read_bytes_total",
    help: "The total number of bytes that have been read.",
    r#type: MetricType::Counter,
};

static WRITE_SAMPLES: Metric = Metric {
    name: "lustre_write_samples_total",
    help: "Total number of writes that have been recorded.",
    r#type: MetricType::Counter,
};
static WRITE_MIN_SIZE_BYTES: Metric = Metric {
    name: "lustre_write_minimum_size_bytes",
    help: "The minimum write size in bytes.",
    r#type: MetricType::Gauge,
};
static WRITE_MAX_SIZE_BYTES: Metric = Metric {
    name: "lustre_write_maximum_size_bytes",
    help: "The maximum write size in bytes.",
    r#type: MetricType::Gauge,
};
static WRITE_BYTES: Metric = Metric {
    name: "lustre_write_bytes_total",
    help: "The total number of bytes that have been written.",
    r#type: MetricType::Counter,
};

pub fn build_ost_stats(
    x: Vec<Stat>,
    target: Target,
    stats_map: &mut BTreeMap<&'static str, PrometheusMetric<'static>>,
) {
    let kind = lustre_collector::TargetVariant::Ost;
    for s in x {
        match s.name.as_str() {
            "read_bytes" => {
                stats_map
                    .get_mut_metric(READ_SAMPLES)
                    .render_and_append_instance(
                        &PrometheusInstance::new()
                            .with_label("component", kind.to_prom_label())
                            .with_label("operation", "read")
                            .with_label("target", target.deref())
                            .with_value(s.samples),
                    );
                s.min.map(|v| {
                    stats_map
                        .get_mut_metric(READ_MIN_SIZE_BYTES)
                        .render_and_append_instance(
                            &PrometheusInstance::new()
                                .with_label("component", kind.to_prom_label())
                                .with_label("operation", "read")
                                .with_label("target", target.deref())
                                .with_value(v),
                        )
                });
                s.max.map(|v| {
                    stats_map
                        .get_mut_metric(READ_MAX_SIZE_BYTES)
                        .render_and_append_instance(
                            &PrometheusInstance::new()
                                .with_label("component", kind.to_prom_label())
                                .with_label("operation", "read")
                                .with_label("target", target.deref())
                                .with_value(v),
                        )
                });
                s.sum.map(|v| {
                    stats_map
                        .get_mut_metric(READ_BYTES)
                        .render_and_append_instance(
                            &PrometheusInstance::new()
                                .with_label("component", kind.to_prom_label())
                                .with_label("operation", "read")
                                .with_label("target", target.deref())
                                .with_value(v),
                        )
                });
            }
            "write_bytes" => {
                stats_map
                    .get_mut_metric(WRITE_SAMPLES)
                    .render_and_append_instance(
                        &PrometheusInstance::new()
                            .with_label("component", kind.to_prom_label())
                            .with_label("operation", "write")
                            .with_label("target", target.deref())
                            .with_value(s.samples),
                    );
                s.min.map(|v| {
                    stats_map
                        .get_mut_metric(WRITE_MIN_SIZE_BYTES)
                        .render_and_append_instance(
                            &PrometheusInstance::new()
                                .with_label("component", kind.to_prom_label())
                                .with_label("operation", "write")
                                .with_label("target", target.deref())
                                .with_value(v),
                        )
                });
                s.max.map(|v| {
                    stats_map
                        .get_mut_metric(WRITE_MAX_SIZE_BYTES)
                        .render_and_append_instance(
                            &PrometheusInstance::new()
                                .with_label("component", kind.to_prom_label())
                                .with_label("operation", "write")
                                .with_label("target", target.deref())
                                .with_value(v),
                        )
                });
                s.sum.map(|v| {
                    stats_map
                        .get_mut_metric(WRITE_BYTES)
                        .render_and_append_instance(
                            &PrometheusInstance::new()
                                .with_label("component", kind.to_prom_label())
                                .with_label("operation", "write")
                                .with_label("target", target.deref())
                                .with_value(v),
                        )
                });
            }
            _x => {
                // Ignore
            }
        }
    }
}

static MDT_STATS_SAMPLES: Metric = Metric {
    name: "lustre_stats_total",
    help: "Number of operations the filesystem has performed.",
    r#type: MetricType::Counter,
};

pub fn build_mdt_stats(
    x: Vec<Stat>,
    target: Target,
    stats_map: &mut BTreeMap<&'static str, PrometheusMetric<'static>>,
) {
    let kind = lustre_collector::TargetVariant::Mdt;
    for s in x {
        stats_map
            .get_mut_metric(MDT_STATS_SAMPLES)
            .render_and_append_instance(
                &PrometheusInstance::new()
                    .with_label("component", kind.to_prom_label())
                    .with_label("operation", s.name.deref())
                    .with_label("target", target.deref())
                    .with_value(s.samples),
            );
    }
}

pub fn build_stats(
    x: TargetStat<Vec<Stat>>,
    stats_map: &mut BTreeMap<&'static str, PrometheusMetric<'static>>,
) {
    let TargetStat {
        kind,
        target,
        value,
        ..
    } = x;

    match kind {
        lustre_collector::TargetVariant::Ost => build_ost_stats(value, target, stats_map),
        lustre_collector::TargetVariant::Mgt => { /*TODO*/ }
        lustre_collector::TargetVariant::Mdt => build_mdt_stats(value, target, stats_map),
    }
}

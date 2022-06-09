use std::{collections::BTreeMap, ops::Deref, time::Duration};

use crate::{Metric, StatsMapExt};
use lustre_collector::{Stat, Target, TargetStat, TargetVariant};
use prometheus_exporter_base::prelude::*;

static READ_SAMPLES: Metric = Metric {
    name: "read_samples_total",
    help: "Total number of reads that have been recorded.",
    r#type: MetricType::Counter,
};
static READ_MIN_SIZE_BYTES: Metric = Metric {
    name: "read_minimum_size_bytes",
    help: "The minimum read size in bytes.",
    r#type: MetricType::Gauge,
};
static READ_MAX_SIZE_BYTES: Metric = Metric {
    name: "read_maximum_size_bytes",
    help: "The maximum read size in bytes.",
    r#type: MetricType::Gauge,
};
static READ_BYTES: Metric = Metric {
    name: "read_bytes_total",
    help: "The total number of bytes that have been read.",
    r#type: MetricType::Counter,
};

static WRITE_SAMPLES: Metric = Metric {
    name: "write_samples_total",
    help: "Total number of writes that have been recorded.",
    r#type: MetricType::Counter,
};
static WRITE_MIN_SIZE_BYTES: Metric = Metric {
    name: "write_minimum_size_bytes",
    help: "The minimum write size in bytes.",
    r#type: MetricType::Gauge,
};
static WRITE_MAX_SIZE_BYTES: Metric = Metric {
    name: "write_maximum_size_bytes",
    help: "The maximum write size in bytes.",
    r#type: MetricType::Gauge,
};
static WRITE_BYTES: Metric = Metric {
    name: "write_bytes_total",
    help: "The total number of bytes that have been written.",
    r#type: MetricType::Counter,
};

pub fn build_ost_stats(
    x: Vec<Stat>,
    kind: TargetVariant,
    target: Target,
    stats_map: &mut BTreeMap<&'static str, PrometheusMetric<'static>>,
    time: Duration,
) {
    for s in x {
        match s.name.as_str() {
            "read_bytes" => {
                stats_map
                    .get_mut_metric(READ_SAMPLES)
                    .render_and_append_instance(
                        &PrometheusInstance::new()
                            .with_label("component", kind.deref())
                            .with_label("operation", "read")
                            .with_label("target", target.deref())
                            .with_value(s.samples)
                            .with_timestamp(time.as_millis()),
                    );
                s.min.map(|v| {
                    stats_map
                        .get_mut_metric(READ_MIN_SIZE_BYTES)
                        .render_and_append_instance(
                            &PrometheusInstance::new()
                                .with_label("component", kind.deref())
                                .with_label("operation", "read")
                                .with_label("target", target.deref())
                                .with_value(v)
                                .with_timestamp(time.as_millis()),
                        )
                });
                s.max.map(|v| {
                    stats_map
                        .get_mut_metric(READ_MAX_SIZE_BYTES)
                        .render_and_append_instance(
                            &PrometheusInstance::new()
                                .with_label("component", kind.deref())
                                .with_label("operation", "read")
                                .with_label("target", target.deref())
                                .with_value(v)
                                .with_timestamp(time.as_millis()),
                        )
                });
                s.sum.map(|v| {
                    stats_map
                        .get_mut_metric(READ_BYTES)
                        .render_and_append_instance(
                            &PrometheusInstance::new()
                                .with_label("component", kind.deref())
                                .with_label("operation", "read")
                                .with_label("target", target.deref())
                                .with_value(v)
                                .with_timestamp(time.as_millis()),
                        )
                });
            }
            "write_bytes" => {
                stats_map
                    .get_mut_metric(WRITE_SAMPLES)
                    .render_and_append_instance(
                        &PrometheusInstance::new()
                            .with_label("component", kind.deref())
                            .with_label("operation", "write")
                            .with_label("target", target.deref())
                            .with_value(s.samples)
                            .with_timestamp(time.as_millis()),
                    );
                s.min.map(|v| {
                    stats_map
                        .get_mut_metric(WRITE_MIN_SIZE_BYTES)
                        .render_and_append_instance(
                            &PrometheusInstance::new()
                                .with_label("component", kind.deref())
                                .with_label("operation", "write")
                                .with_label("target", target.deref())
                                .with_value(v)
                                .with_timestamp(time.as_millis()),
                        )
                });
                s.max.map(|v| {
                    stats_map
                        .get_mut_metric(WRITE_MAX_SIZE_BYTES)
                        .render_and_append_instance(
                            &PrometheusInstance::new()
                                .with_label("component", kind.deref())
                                .with_label("operation", "write")
                                .with_label("target", target.deref())
                                .with_value(v)
                                .with_timestamp(time.as_millis()),
                        )
                });
                s.sum.map(|v| {
                    stats_map
                        .get_mut_metric(WRITE_BYTES)
                        .render_and_append_instance(
                            &PrometheusInstance::new()
                                .with_label("component", kind.deref())
                                .with_label("operation", "write")
                                .with_label("target", target.deref())
                                .with_value(v)
                                .with_timestamp(time.as_millis()),
                        )
                });
            }
            x => println!("{x}"),
        }
    }
}

pub fn build_stats(
    x: TargetStat<Vec<Stat>>,
    stats_map: &mut BTreeMap<&'static str, PrometheusMetric<'static>>,
    time: Duration,
) {
    let TargetStat {
        kind,
        target,
        value,
        ..
    } = x;

    match kind {
        lustre_collector::TargetVariant::Ost => {
            build_ost_stats(value, kind, target, stats_map, time)
        }
        lustre_collector::TargetVariant::Mgt => { /*TODO*/ }
        lustre_collector::TargetVariant::Mdt => { /*TODO*/ }
    }
}

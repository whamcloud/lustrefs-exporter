use crate::{LabelProm, Metric, StatsMapExt};
use lustre_collector::{ExportStats, MdsStat, Stat, Target, TargetStat};
use prometheus_exporter_base::prelude::*;
use std::{collections::BTreeMap, ops::Deref};

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

static MDS_STATS: Metric = Metric {
    name: "lustre_mds_mdt_stats",
    help: "MDS mdt stats",
    r#type: MetricType::Gauge,
};

static MDS_FLD_STATS: Metric = Metric {
    name: "lustre_mds_mdt_fld_stats",
    help: "MDS mdt_fld stats",
    r#type: MetricType::Gauge,
};

static MDS_IO_STATS: Metric = Metric {
    name: "lustre_mds_mdt_io_stats",
    help: "MDS mdt_io stats",
    r#type: MetricType::Gauge,
};

static MDS_OUT_STATS: Metric = Metric {
    name: "lustre_mds_mdt_out_stats",
    help: "MDS mdt_out stats",
    r#type: MetricType::Gauge,
};

static MDS_READPAGE_STATS: Metric = Metric {
    name: "lustre_mds_mdt_readpage_stats",
    help: "MDS mdt_readpage stats",
    r#type: MetricType::Gauge,
};

static MDS_SEQM_STATS: Metric = Metric {
    name: "lustre_mds_mdt_seqm_stats",
    help: "MDS mdt_seqm stats",
    r#type: MetricType::Gauge,
};

static MDS_SEQS_STATS: Metric = Metric {
    name: "lustre_mds_mdt_seqs_stats",
    help: "MDS mdt_seqs stats",
    r#type: MetricType::Gauge,
};

static MDS_SETATTR_STATS: Metric = Metric {
    name: "lustre_mds_mdt_setattr_stats",
    help: "MDS mdt_setattr stats",
    r#type: MetricType::Gauge,
};

pub fn build_mds_stats(
    x: MdsStat,
    stats_map: &mut BTreeMap<&'static str, PrometheusMetric<'static>>,
) {
    let MdsStat { param, stats } = x;

    for x in stats {
        let Stat {
            name,
            units,
            samples,
            ..
        } = x;

        let metric = match param.0.as_str() {
            "mdt" => stats_map.get_mut_metric(MDS_STATS),
            "mdt_fld" => stats_map.get_mut_metric(MDS_FLD_STATS),
            "mdt_io" => stats_map.get_mut_metric(MDS_IO_STATS),
            "mdt_out" => stats_map.get_mut_metric(MDS_OUT_STATS),
            "mdt_readpage" => stats_map.get_mut_metric(MDS_READPAGE_STATS),
            "mdt_seqm" => stats_map.get_mut_metric(MDS_SEQM_STATS),
            "mdt_seqs" => stats_map.get_mut_metric(MDS_SEQS_STATS),
            "mdt_setattr" => stats_map.get_mut_metric(MDS_SETATTR_STATS),
            _ => continue,
        };

        let stat = PrometheusInstance::new()
            .with_label("operation", name.as_str())
            .with_label("units", units.as_str())
            .with_value(samples);

        metric.render_and_append_instance(&stat);
    }
}

static EXPORT_STATS: Metric = Metric {
    name: "lustre_client_export_stats",
    help: "Number of operations the target has performed per export.",
    r#type: MetricType::Counter,
};

pub fn build_export_stats(
    x: TargetStat<Vec<ExportStats>>,
    stats_map: &mut BTreeMap<&'static str, PrometheusMetric<'static>>,
) {
    let TargetStat {
        kind,
        value: export_stats,
        param,
        target,
    } = x;

    for e in export_stats {
        let ExportStats { nid, stats } = e;
        for s in stats {
            let Stat {
                name,
                units,
                samples,
                ..
            } = s;
            let metric = match param.0.as_str() {
                "exports" => stats_map.get_mut_metric(EXPORT_STATS),
                _ => continue,
            };

            let stat = PrometheusInstance::new()
                .with_label("component", kind.to_prom_label())
                .with_label("target", target.deref())
                .with_label("nid", nid.as_str())
                .with_label("name", name.as_str())
                .with_label("units", units.as_str())
                .with_value(samples);

            metric.render_and_append_instance(&stat);
        }
    }
}

pub mod opentelemetry {
    use std::ops::Deref;

    use lustre_collector::{ExportStats, MdsStat, Stat, Target, TargetStat, TargetVariant};
    use opentelemetry::{
        metrics::{Counter, Gauge, Meter},
        KeyValue,
    };

    use crate::LabelProm as _;

    #[derive(Debug)]
    pub struct OpenTelemetryMetricsStats {
        // OST metrics
        pub read_samples_total: Counter<u64>,
        pub read_minimum_size_bytes: Gauge<u64>,
        pub read_maximum_size_bytes: Gauge<u64>,
        pub read_bytes_total: Counter<u64>,
        pub write_samples_total: Counter<u64>,
        pub write_minimum_size_bytes: Gauge<u64>,
        pub write_maximum_size_bytes: Gauge<u64>,
        pub write_bytes_total: Counter<u64>,

        // MDT metrics
        pub stats_total: Counter<u64>,

        // MDS metrics
        pub mds_mdt_stats: Gauge<u64>,
        pub mds_mdt_fld_stats: Gauge<u64>,
        pub mds_mdt_io_stats: Gauge<u64>,
        pub mds_mdt_out_stats: Gauge<u64>,
        pub mds_mdt_readpage_stats: Gauge<u64>,
        pub mds_mdt_seqm_stats: Gauge<u64>,
        pub mds_mdt_seqs_stats: Gauge<u64>,
        pub mds_mdt_setattr_stats: Gauge<u64>,

        // Export metrics
        pub client_export_stats: Counter<u64>,
    }

    impl OpenTelemetryMetricsStats {
        pub fn new(meter: &Meter) -> Self {
            OpenTelemetryMetricsStats {
                // OST metrics
                read_samples_total: meter
                    .u64_counter("lustre_read_samples_total")
                    .with_description("Total number of reads that have been recorded.")
                    .build(),
                read_minimum_size_bytes: meter
                    .u64_gauge("lustre_read_minimum_size_bytes")
                    .with_description("The minimum read size in bytes.")
                    .build(),
                read_maximum_size_bytes: meter
                    .u64_gauge("lustre_read_maximum_size_bytes")
                    .with_description("The maximum read size in bytes.")
                    .build(),
                read_bytes_total: meter
                    .u64_counter("lustre_read_bytes_total")
                    .with_description("The total number of bytes that have been read.")
                    .build(),
                write_samples_total: meter
                    .u64_counter("lustre_write_samples_total")
                    .with_description("Total number of writes that have been recorded.")
                    .build(),
                write_minimum_size_bytes: meter
                    .u64_gauge("lustre_write_minimum_size_bytes")
                    .with_description("The minimum write size in bytes.")
                    .build(),
                write_maximum_size_bytes: meter
                    .u64_gauge("lustre_write_maximum_size_bytes")
                    .with_description("The maximum write size in bytes.")
                    .build(),
                write_bytes_total: meter
                    .u64_counter("lustre_write_bytes_total")
                    .with_description("The total number of bytes that have been written.")
                    .build(),

                // MDT metrics
                stats_total: meter
                    .u64_counter("lustre_stats_total")
                    .with_description("Number of operations the filesystem has performed.")
                    .build(),

                // MDS metrics
                mds_mdt_stats: meter
                    .u64_gauge("lustre_mds_mdt_stats")
                    .with_description("MDS mdt stats")
                    .build(),
                mds_mdt_fld_stats: meter
                    .u64_gauge("lustre_mds_mdt_fld_stats")
                    .with_description("MDS mdt_fld stats")
                    .build(),
                mds_mdt_io_stats: meter
                    .u64_gauge("lustre_mds_mdt_io_stats")
                    .with_description("MDS mdt_io stats")
                    .build(),
                mds_mdt_out_stats: meter
                    .u64_gauge("lustre_mds_mdt_out_stats")
                    .with_description("MDS mdt_out stats")
                    .build(),
                mds_mdt_readpage_stats: meter
                    .u64_gauge("lustre_mds_mdt_readpage_stats")
                    .with_description("MDS mdt_readpage stats")
                    .build(),
                mds_mdt_seqm_stats: meter
                    .u64_gauge("lustre_mds_mdt_seqm_stats")
                    .with_description("MDS mdt_seqm stats")
                    .build(),
                mds_mdt_seqs_stats: meter
                    .u64_gauge("lustre_mds_mdt_seqs_stats")
                    .with_description("MDS mdt_seqs stats")
                    .build(),
                mds_mdt_setattr_stats: meter
                    .u64_gauge("lustre_mds_mdt_setattr_stats")
                    .with_description("MDS mdt_setattr stats")
                    .build(),

                // Export metrics
                client_export_stats: meter
                    .u64_counter("lustre_client_export_stats")
                    .with_description("Number of operations the target has performed per export.")
                    .build(),
            }
        }
    }

    pub fn build_ost_stats(
        stats: &[Stat],
        target: &Target,
        otel_stats: &OpenTelemetryMetricsStats,
    ) {
        let kind = TargetVariant::Ost;
        for s in stats {
            match s.name.as_str() {
                "read_bytes" => {
                    let read_labels = &[
                        KeyValue::new("component", kind.to_prom_label().to_string()),
                        KeyValue::new("operation", "read"),
                        KeyValue::new("target", target.deref().to_string()),
                    ];

                    otel_stats.read_samples_total.add(s.samples, read_labels);
                    if let Some(min) = s.min {
                        otel_stats.read_minimum_size_bytes.record(min, read_labels);
                    }
                    if let Some(max) = s.max {
                        otel_stats.read_maximum_size_bytes.record(max, read_labels);
                    }
                    if let Some(sum) = s.sum {
                        otel_stats.read_bytes_total.add(sum, read_labels);
                    }
                }
                "write_bytes" => {
                    let write_labels = &[
                        KeyValue::new("component", kind.to_prom_label().to_string()),
                        KeyValue::new("operation", "write"),
                        KeyValue::new("target", target.deref().to_string()),
                    ];

                    otel_stats.write_samples_total.add(s.samples, write_labels);
                    if let Some(min) = s.min {
                        otel_stats
                            .write_minimum_size_bytes
                            .record(min, write_labels);
                    }
                    if let Some(max) = s.max {
                        otel_stats
                            .write_maximum_size_bytes
                            .record(max, write_labels);
                    }
                    if let Some(sum) = s.sum {
                        otel_stats.write_bytes_total.add(sum, write_labels);
                    }
                }
                _ => {
                    // Ignore other stats
                }
            }
        }
    }

    pub fn build_mdt_stats(
        stats: &[Stat],
        target: &Target,
        otel_stats: &OpenTelemetryMetricsStats,
    ) {
        let kind = TargetVariant::Mdt;
        for s in stats {
            let labels = &[
                KeyValue::new("component", kind.to_prom_label().to_string()),
                KeyValue::new("operation", s.name.deref().to_string()),
                KeyValue::new("target", target.deref().to_string()),
            ];

            otel_stats.stats_total.add(s.samples, labels);
        }
    }

    pub fn build_stats(x: &TargetStat<Vec<Stat>>, otel_stats: &OpenTelemetryMetricsStats) {
        let TargetStat {
            kind,
            target,
            value,
            ..
        } = x;

        match kind {
            TargetVariant::Ost => build_ost_stats(value, target, otel_stats),
            TargetVariant::Mgt => { /* TODO */ }
            TargetVariant::Mdt => build_mdt_stats(value, target, otel_stats),
        }
    }

    pub fn build_mds_stats(x: &MdsStat, otel_stats: &OpenTelemetryMetricsStats) {
        let MdsStat { param, stats } = x;

        for stat in stats {
            let labels = &[
                KeyValue::new("operation", stat.name.as_str().to_string()),
                KeyValue::new("units", stat.units.as_str().to_string()),
            ];

            match param.0.as_str() {
                "mdt" => otel_stats.mds_mdt_stats.record(stat.samples, labels),
                "mdt_fld" => otel_stats.mds_mdt_fld_stats.record(stat.samples, labels),
                "mdt_io" => otel_stats.mds_mdt_io_stats.record(stat.samples, labels),
                "mdt_out" => otel_stats.mds_mdt_out_stats.record(stat.samples, labels),
                "mdt_readpage" => otel_stats
                    .mds_mdt_readpage_stats
                    .record(stat.samples, labels),
                "mdt_seqm" => otel_stats.mds_mdt_seqm_stats.record(stat.samples, labels),
                "mdt_seqs" => otel_stats.mds_mdt_seqs_stats.record(stat.samples, labels),
                "mdt_setattr" => otel_stats
                    .mds_mdt_setattr_stats
                    .record(stat.samples, labels),
                _ => {}
            }
        }
    }

    pub fn build_export_stats(
        x: &TargetStat<Vec<ExportStats>>,
        otel_stats: &OpenTelemetryMetricsStats,
    ) {
        let TargetStat {
            kind,
            target,
            param,
            value: export_stats,
        } = x;

        if param.0.as_str() != "exports" {
            return;
        }

        for e in export_stats {
            let ExportStats { nid, stats } = e;
            for s in stats {
                let labels = &[
                    KeyValue::new("component", kind.to_prom_label().to_string()),
                    KeyValue::new("target", target.deref().to_string()),
                    KeyValue::new("nid", nid.as_str().to_string()),
                    KeyValue::new("name", s.name.as_str().to_string()),
                    KeyValue::new("units", s.units.as_str().to_string()),
                ];

                otel_stats.client_export_stats.add(s.samples, labels);
            }
        }
    }
}

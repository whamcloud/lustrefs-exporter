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

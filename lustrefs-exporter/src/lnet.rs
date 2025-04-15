use std::collections::BTreeMap;

use lustre_collector::LNetStats;
use prometheus_exporter_base::prelude::*;

use crate::{Metric, StatsMapExt, ToMetricInst};

static SEND_COUNT: Metric = Metric {
    name: "lustre_send_count_total",
    help: "Total number of messages that have been sent",
    r#type: MetricType::Counter,
};
static RECEIVE_COUNT: Metric = Metric {
    name: "lustre_receive_count_total",
    help: "Total number of messages that have been received",
    r#type: MetricType::Counter,
};
static DROP_COUNT: Metric = Metric {
    name: "lustre_drop_count_total",
    help: "Total number of messages that have been dropped",
    r#type: MetricType::Counter,
};

static SEND_BYTES: Metric = Metric {
    name: "lustre_send_bytes_total",
    help: "Total number of bytes that have been sent",
    r#type: MetricType::Counter,
};
static RECEIVE_BYTES: Metric = Metric {
    name: "lustre_receive_bytes_total",
    help: "Total number of bytes that have been received",
    r#type: MetricType::Counter,
};
static DROP_BYTES: Metric = Metric {
    name: "lustre_drop_bytes_total",
    help: "Total number of bytes that have been dropped",
    r#type: MetricType::Counter,
};

pub fn build_lnet_stats(
    x: LNetStats,
    stats_map: &mut BTreeMap<&'static str, PrometheusMetric<'static>>,
) {
    match x {
        LNetStats::SendCount(x) => {
            stats_map
                .get_mut_metric(SEND_COUNT)
                .render_and_append_instance(&x.to_metric_inst());
        }
        LNetStats::RecvCount(x) => {
            stats_map
                .get_mut_metric(RECEIVE_COUNT)
                .render_and_append_instance(&x.to_metric_inst());
        }
        LNetStats::DropCount(x) => {
            stats_map
                .get_mut_metric(DROP_COUNT)
                .render_and_append_instance(&x.to_metric_inst());
        }
        LNetStats::SendLength(x) => {
            stats_map
                .get_mut_metric(SEND_BYTES)
                .render_and_append_instance(&x.to_metric_inst());
        }
        LNetStats::RecvLength(x) => {
            stats_map
                .get_mut_metric(RECEIVE_BYTES)
                .render_and_append_instance(&x.to_metric_inst());
        }
        LNetStats::DropLength(x) => {
            stats_map
                .get_mut_metric(DROP_BYTES)
                .render_and_append_instance(&x.to_metric_inst());
        }
    };
}

pub mod opentelemetry {
    use lustre_collector::{LNetStat, LNetStatGlobal, LNetStats};
    use opentelemetry::metrics::{Counter, Meter};

    #[derive(Debug)]
    pub struct OpenTelemetryMetricsLnet {
        pub send_count_total: Counter<u64>,
        pub receive_count_total: Counter<u64>,
        pub drop_count_total: Counter<u64>,
        pub send_bytes_total: Counter<u64>,
        pub receive_bytes_total: Counter<u64>,
        pub drop_bytes_total: Counter<u64>,
    }

    impl OpenTelemetryMetricsLnet {
        pub fn new(meter: &Meter) -> Self {
            OpenTelemetryMetricsLnet {
                send_count_total: meter
                    .u64_counter("lustre_send_count_total")
                    .with_description("Total number of messages that have been sent")
                    .build(),
                receive_count_total: meter
                    .u64_counter("lustre_receive_count_total")
                    .with_description("Total number of messages that have been received")
                    .build(),
                drop_count_total: meter
                    .u64_counter("lustre_drop_count_total")
                    .with_description("Total number of messages that have been dropped")
                    .build(),
                send_bytes_total: meter
                    .u64_counter("lustre_send_bytes_total")
                    .with_description("Total number of bytes that have been sent")
                    .build(),
                receive_bytes_total: meter
                    .u64_counter("lustre_receive_bytes_total")
                    .with_description("Total number of bytes that have been received")
                    .build(),
                drop_bytes_total: meter
                    .u64_counter("lustre_drop_bytes_total")
                    .with_description("Total number of bytes that have been dropped")
                    .build(),
            }
        }
    }

    fn record_lnet_stat(stat: &LNetStat<i64>, counter: &Counter<u64>) {
        // let labels = &[
        //     KeyValue::new("net_dev", stat.net_dev.to_string()),
        //     KeyValue::new("portal", format!("{}", stat.portal)),
        // ];
        let labels = &[];
        counter.add(stat.value.try_into().unwrap_or(0), labels);
    }

    fn record_lnet_stat_global(stat: &LNetStatGlobal<i64>, counter: &Counter<u64>) {
        // let labels = &[
        //     KeyValue::new("net_dev", stat.net_dev.to_string()),
        //     KeyValue::new("portal", format!("{}", stat.portal)),
        // ];
        let labels = &[];
        counter.add(stat.value.try_into().unwrap_or(0), labels);
    }

    pub fn build_lnet_stats(x: &LNetStats, otel_lnet: &OpenTelemetryMetricsLnet) {
        match x {
            LNetStats::SendCount(stat) => {
                record_lnet_stat(stat, &otel_lnet.send_count_total);
            }
            LNetStats::RecvCount(stat) => {
                record_lnet_stat(stat, &otel_lnet.receive_count_total);
            }
            LNetStats::DropCount(stat) => {
                record_lnet_stat(stat, &otel_lnet.drop_count_total);
            }
            LNetStats::SendLength(stat) => {
                record_lnet_stat_global(stat, &otel_lnet.send_bytes_total);
            }
            LNetStats::RecvLength(stat) => {
                record_lnet_stat_global(stat, &otel_lnet.receive_bytes_total);
            }
            LNetStats::DropLength(stat) => {
                record_lnet_stat_global(stat, &otel_lnet.drop_bytes_total);
            }
        }
    }
}

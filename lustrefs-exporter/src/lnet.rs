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

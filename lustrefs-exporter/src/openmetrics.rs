use lustre_collector::Record;
use opentelemetry::metrics::Meter;

use crate::{
    brw_stats::opentelemetry::build_target_stats, host::build_host_stats, lnet::build_lnet_stats,
    quota::opentelemetry::OpenTelemetryMetricsQuota, service::build_service_stats,
};

#[derive(Debug)]
pub struct OpenTelemetryMetrics {
    pub quota: OpenTelemetryMetricsQuota,
}

impl OpenTelemetryMetrics {
    pub fn new(meter: Meter) -> Self {
        OpenTelemetryMetrics {
            quota: OpenTelemetryMetricsQuota::new(meter),
        }
    }
}

pub fn build_lustre_stats(output: &Vec<Record>, otel: OpenTelemetryMetrics) {
    for x in output {
        match x {
            // lustre_collector::Record::Host(x) => {
            //     build_host_stats(x, &mut stats_map);
            // }
            // lustre_collector::Record::Node(_) => {}
            // lustre_collector::Record::LNetStat(x) => {
            //     build_lnet_stats(x, &mut stats_map);
            // }
            lustre_collector::Record::Target(x) => {
                build_target_stats(x, &otel);
            }
            // lustre_collector::Record::LustreService(x) => {
            //     build_service_stats(x, &mut stats_map);
            // }
            _ => {}
        }
    }
}

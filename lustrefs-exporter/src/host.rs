use crate::{Metric, StatsMapExt, ToMetricInst};
use lustre_collector::HostStats;
use prometheus_exporter_base::prelude::*;
use std::collections::BTreeMap;
use std::ops::Deref;

static LUSTRE_TARGETS_HEALTHY: Metric = Metric {
    name: "lustre_health_healthy",
    help: "Indicates whether the Lustre target is healthy or not. 1 is healthy, 0 is unhealthy.",
    r#type: MetricType::Gauge,
};

static LNET_MEM_USED_SAMPLES: Metric = Metric {
    name: "lustre_lnet_mem_used",
    help: "Gives information about Lustre LNet memory usage.",
    r#type: MetricType::Gauge,
};

static MEM_USED_SAMPLES: Metric = Metric {
    name: "lustre_mem_used",
    help: "Gives information about Lustre memory usage.",
    r#type: MetricType::Gauge,
};

static MEM_USED_MAX_SAMPLES: Metric = Metric {
    name: "lustre_mem_used_max",
    help: "Gives information about Lustre maximum memory usage.",
    r#type: MetricType::Gauge,
};

pub fn build_host_stats(
    x: HostStats,
    stats_map: &mut BTreeMap<&'static str, PrometheusMetric<'static>>,
) {
    match x {
        HostStats::HealthCheck(x) => {
            let healthy = x.value.healthy;
            let metric = PrometheusInstance::new().with_value(i32::from(healthy));

            stats_map
                .get_mut_metric(LUSTRE_TARGETS_HEALTHY)
                .render_and_append_instance(&metric);

            if !x.value.targets.is_empty() {
                for target in x.value.targets {
                    let metric = PrometheusInstance::new()
                        .with_label("target", target.deref())
                        .with_value(i32::from(healthy));

                    stats_map
                        .get_mut_metric(LUSTRE_TARGETS_HEALTHY)
                        .render_and_append_instance(&metric);
                }
            }
        }
        HostStats::LNetMemUsed(x) => {
            stats_map
                .get_mut_metric(LNET_MEM_USED_SAMPLES)
                .render_and_append_instance(&x.to_metric_inst());
        }
        HostStats::Memused(x) => {
            stats_map
                .get_mut_metric(MEM_USED_SAMPLES)
                .render_and_append_instance(&x.to_metric_inst());
        }
        HostStats::MemusedMax(x) => {
            stats_map
                .get_mut_metric(MEM_USED_MAX_SAMPLES)
                .render_and_append_instance(&x.to_metric_inst());
        }
    };
}

pub mod opentelemetry {
    use lustre_collector::HostStats;
    use opentelemetry::{
        metrics::{Gauge, Meter},
        KeyValue,
    };
    use std::ops::Deref;

    #[derive(Debug)]
    pub struct OpenTelemetryMetricsHost {
        pub lustre_targets_healthy: Gauge<u64>,
        pub lnet_mem_used: Gauge<u64>,
        pub mem_used: Gauge<u64>,
        pub mem_used_max: Gauge<u64>,
    }

    impl OpenTelemetryMetricsHost {
        pub fn new(meter: &Meter) -> Self {
            OpenTelemetryMetricsHost {
                lustre_targets_healthy: meter
                    .u64_gauge("lustre_health_healthy")
                    .with_description("Indicates whether the Lustre target is healthy or not. 1 is healthy, 0 is unhealthy.")
                    .build(),
                lnet_mem_used: meter
                    .u64_gauge("lustre_lnet_mem_used")
                    .with_description("Gives information about Lustre LNet memory usage.")
                    .build(),
                mem_used: meter
                    .u64_gauge("lustre_mem_used")
                    .with_description("Gives information about Lustre memory usage.")
                    .build(),
                mem_used_max: meter
                    .u64_gauge("lustre_mem_used_max")
                    .with_description("Gives information about Lustre maximum memory usage.")
                    .build(),
            }
        }
    }

    pub fn build_host_stats(x: &HostStats, otel_host: &OpenTelemetryMetricsHost) {
        match x {
            HostStats::HealthCheck(x) => {
                let healthy = x.value.healthy;
                otel_host
                    .lustre_targets_healthy
                    .record(if healthy { 1 } else { 0 }, &[]);

                for target in &x.value.targets {
                    otel_host.lustre_targets_healthy.record(
                        if healthy { 1 } else { 0 },
                        &[KeyValue::new("target", target.deref().to_string())],
                    );
                }
            }
            HostStats::LNetMemUsed(x) => {
                otel_host.lnet_mem_used.record(x.value, &[]);
            }
            HostStats::Memused(x) => {
                otel_host.mem_used.record(x.value, &[]);
            }
            HostStats::MemusedMax(x) => {
                otel_host.mem_used_max.record(x.value, &[]);
            }
        }
    }
}

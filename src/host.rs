use crate::{Metric, StatsMapExt, ToMetricInst};
use lustre_collector::HostStats;
use prometheus_exporter_base::prelude::*;
use std::collections::BTreeMap;
use std::ops::Deref;

static LUSTRE_TARGETS_HEALTHY: Metric = Metric {
    name: "lustre_health_healthy",
    help:
        "Indicates whether the Lustre target is healthy or not. 1 is healthy, 0 is unhealthy.",
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
            let metric = PrometheusInstance::new().with_value(healthy as i32);

            stats_map
                .get_mut_metric(LUSTRE_TARGETS_HEALTHY)
                .render_and_append_instance(&metric);

            if !x.value.targets.is_empty() {
                for target in x.value.targets {
                    let metric = PrometheusInstance::new()
                        .with_label("target", target.deref())
                        .with_value(healthy as i32);

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

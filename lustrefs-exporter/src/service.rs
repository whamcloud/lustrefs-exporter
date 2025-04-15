use crate::{Metric, StatsMapExt};
use lustre_collector::LustreServiceStats;
use prometheus_exporter_base::prelude::*;
use std::{collections::BTreeMap, ops::Deref};

static LDLM_CANCELD_STATS_SAMPLES: Metric = Metric {
    name: "lustre_ldlm_canceld_stats",
    help: "Gives information about LDLM Canceld service.",
    r#type: MetricType::Counter,
};

static LDLM_CBD_STATS_SAMPLES: Metric = Metric {
    name: "lustre_ldlm_cbd_stats",
    help: "Gives information about LDLM Callback service.",
    r#type: MetricType::Counter,
};

pub fn build_service_stats(
    x: LustreServiceStats,
    stats_map: &mut BTreeMap<&'static str, PrometheusMetric<'static>>,
) {
    match x {
        LustreServiceStats::LdlmCanceld(xs) => {
            for s in xs {
                stats_map
                    .get_mut_metric(LDLM_CANCELD_STATS_SAMPLES)
                    .render_and_append_instance(
                        &PrometheusInstance::new()
                            .with_label("operation", s.name.deref())
                            .with_value(s.samples),
                    );
            }
        }
        LustreServiceStats::LdlmCbd(xs) => {
            for s in xs {
                stats_map
                    .get_mut_metric(LDLM_CBD_STATS_SAMPLES)
                    .render_and_append_instance(
                        &PrometheusInstance::new()
                            .with_label("operation", s.name.deref())
                            .with_value(s.samples),
                    );
            }
        }
    };
}

pub mod opentelemetry {
    use lustre_collector::LustreServiceStats;
    use opentelemetry::{
        metrics::{Counter, Meter},
        KeyValue,
    };
    use std::ops::Deref;

    #[derive(Debug)]
    pub struct OpenTelemetryMetricsService {
        pub ldlm_canceld_stats: Counter<u64>,
        pub ldlm_cbd_stats: Counter<u64>,
    }

    impl OpenTelemetryMetricsService {
        pub fn new(meter: &Meter) -> Self {
            OpenTelemetryMetricsService {
                ldlm_canceld_stats: meter
                    .u64_counter("lustre_ldlm_canceld_stats")
                    .with_description("Gives information about LDLM Canceld service.")
                    .build(),
                ldlm_cbd_stats: meter
                    .u64_counter("lustre_ldlm_cbd_stats")
                    .with_description("Gives information about LDLM Callback service.")
                    .build(),
            }
        }
    }

    pub fn build_service_stats(x: &LustreServiceStats, otel_service: &OpenTelemetryMetricsService) {
        match x {
            LustreServiceStats::LdlmCanceld(xs) => {
                for s in xs {
                    otel_service.ldlm_canceld_stats.add(
                        s.samples,
                        &[KeyValue::new("operation", s.name.deref().to_string())],
                    );
                }
            }
            LustreServiceStats::LdlmCbd(xs) => {
                for s in xs {
                    otel_service.ldlm_cbd_stats.add(
                        s.samples,
                        &[KeyValue::new("operation", s.name.deref().to_string())],
                    );
                }
            }
        }
    }
}

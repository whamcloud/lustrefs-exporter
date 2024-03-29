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

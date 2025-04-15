// Copyright (c) 2024 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use std::{collections::BTreeMap, ops::Deref};

use lustre_collector::LliteStat;
use prometheus_exporter_base::prelude::*;

use crate::{Metric, StatsMapExt};

static LLITE_STATS_SAMPLES: Metric = Metric {
    name: "lustre_client_stats",
    help: "Lustre client interface stats.",
    r#type: MetricType::Gauge,
};

pub fn build_llite_stats(
    x: LliteStat,
    stats_map: &mut BTreeMap<&'static str, PrometheusMetric<'static>>,
) {
    let LliteStat {
        target,
        param: _,
        stats,
    } = x;

    for stat in stats {
        stats_map
            .get_mut_metric(LLITE_STATS_SAMPLES)
            .render_and_append_instance(
                &PrometheusInstance::new()
                    .with_label("operation", stat.name.deref())
                    .with_label("target", target.deref())
                    .with_value(stat.samples),
            );
    }
}

pub mod opentelemetry {
    use std::ops::Deref;

    use lustre_collector::LliteStat;
    use opentelemetry::{
        metrics::{Gauge, Meter},
        KeyValue,
    };

    #[derive(Debug)]
    pub struct OpenTelemetryMetricsLlite {
        pub client_stats: Gauge<u64>,
    }

    impl OpenTelemetryMetricsLlite {
        pub fn new(meter: &Meter) -> Self {
            OpenTelemetryMetricsLlite {
                client_stats: meter
                    .u64_gauge("lustre_client_stats")
                    .with_description("Lustre client interface stats.")
                    .build(),
            }
        }
    }

    pub fn build_llite_stats(x: &LliteStat, otel_llite: &OpenTelemetryMetricsLlite) {
        let LliteStat {
            target,
            param: _,
            stats,
        } = x;

        for stat in stats {
            otel_llite.client_stats.record(
                stat.samples,
                &[
                    KeyValue::new("operation", stat.name.deref().to_string()),
                    KeyValue::new("target", target.deref().to_string()),
                ],
            );
        }
    }
}

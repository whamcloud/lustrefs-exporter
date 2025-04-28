// Copyright (c) 2024 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.
pub mod opentelemetry {
    use std::ops::Deref;

    use lustre_collector::LliteStat;
    use opentelemetry::{
        metrics::{Counter, Meter},
        KeyValue,
    };

    #[derive(Debug)]
    pub struct OpenTelemetryMetricsLlite {
        pub client_stats: Counter<u64>,
    }

    impl OpenTelemetryMetricsLlite {
        pub fn new(meter: &Meter) -> Self {
            OpenTelemetryMetricsLlite {
                client_stats: meter
                    .u64_counter("lustre_client_stats")
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
            otel_llite.client_stats.add(
                stat.samples,
                &[
                    KeyValue::new("operation", stat.name.deref().to_string()),
                    KeyValue::new("target", target.deref().to_string()),
                ],
            );
        }
    }
}

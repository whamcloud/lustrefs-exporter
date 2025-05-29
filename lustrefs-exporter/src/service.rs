// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

pub mod opentelemetry {
    use lustre_collector::LustreServiceStats;
    use opentelemetry::{
        KeyValue,
        metrics::{Counter, Meter},
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

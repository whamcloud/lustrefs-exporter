// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

pub mod opentelemetry {
    use lustre_collector::{LNetStat, LNetStatGlobal, LNetStats};
    use opentelemetry::{
        metrics::{Counter, Meter},
        KeyValue,
    };

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
        let labels = &[KeyValue::new("nid", stat.nid.to_string())];
        counter.add(stat.value.try_into().unwrap_or(0), labels);
    }

    fn record_lnet_stat_global(stat: &LNetStatGlobal<i64>, counter: &Counter<u64>) {
        counter.add(stat.value.try_into().unwrap_or(0), &[]);
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

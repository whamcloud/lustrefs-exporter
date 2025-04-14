// Copyright (c) 2024 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{LabelProm, Metric, StatsMapExt};
use lustre_collector::{QuotaStats, QuotaStatsOsd, TargetQuotaStat, TargetStat};
use prometheus_exporter_base::prelude::*;
use std::{collections::BTreeMap, ops::Deref};

pub mod opentelemetry {
    use std::ops::Deref as _;

    use lustre_collector::{QuotaStats, QuotaStatsOsd, TargetQuotaStat, TargetStat};
    use opentelemetry::{
        metrics::{Gauge, Meter},
        KeyValue,
    };

    #[derive(Debug)]
    pub struct OpenTelemetryMetricsQuota {
        pub quota_hard: Gauge<u64>,
        pub quota_soft: Gauge<u64>,
        pub quota_granted: Gauge<u64>,
        pub quota_used_kbytes: Gauge<u64>,
        pub quota_used_inodes: Gauge<u64>,
    }

    impl OpenTelemetryMetricsQuota {
        pub fn new(meter: Meter) -> Self {
            OpenTelemetryMetricsQuota {
                quota_hard: meter
                    .u64_gauge("lustre_quota_hard")
                    .with_description("The hard quota for a given component.")
                    .with_unit("bytes")
                    .build(),
                quota_soft: meter
                    .u64_gauge("lustre_quota_soft")
                    .with_description("The soft quota for a given component.")
                    .with_unit("bytes")
                    .build(),
                quota_granted: meter
                    .u64_gauge("lustre_quota_granted")
                    .with_description("The granted quota for a given component.")
                    .with_unit("bytes")
                    .build(),
                quota_used_kbytes: meter
                    .u64_gauge("lustre_quota_used_kbytes")
                    .with_description("The amount of kbytes used by quota.")
                    .with_unit("kbytes")
                    .build(),
                quota_used_inodes: meter
                    .u64_gauge("lustre_quota_used_inodes")
                    .with_description("The amount of inodes used by quota.")
                    .with_unit("inodes")
                    .build(),
            }
        }
    }

    pub fn build_quota_stats(
        x: &TargetQuotaStat<QuotaStats>,
        otel_quota: &OpenTelemetryMetricsQuota,
    ) {
        let TargetQuotaStat {
            target,
            value,
            pool,
            manager,
            param,
            ..
        } = x;

        for s in &value.stats {
            let pool = pool.deref().to_string();
            let pool = if pool == "0x0" { "".to_string() } else { pool };
            let accounting = match param.deref() {
                "usr" => "user".to_string(),
                "grp" => "group".to_string(),
                "prj" => "project".to_string(),
                _ => param.to_string(),
            };
            otel_quota.quota_hard.record(
                s.limits.hard,
                &[
                    KeyValue::new("target", target.to_string()),
                    KeyValue::new("pool", pool.clone()),
                    KeyValue::new("accounting", accounting.clone()),
                    KeyValue::new("manager", manager.to_string()),
                    KeyValue::new("id", s.id.to_string()),
                ],
            );
            otel_quota.quota_soft.record(
                s.limits.soft,
                &[
                    KeyValue::new("target", target.to_string()),
                    KeyValue::new("pool", pool.clone()),
                    KeyValue::new("accounting", accounting.clone()),
                    KeyValue::new("manager", manager.to_string()),
                    KeyValue::new("id", s.id.to_string()),
                ],
            );
            otel_quota.quota_granted.record(
                s.limits.granted,
                &[
                    KeyValue::new("target", target.to_string()),
                    KeyValue::new("pool", pool),
                    KeyValue::new("accounting", accounting),
                    KeyValue::new("manager", manager.to_string()),
                    KeyValue::new("id", s.id.to_string()),
                ],
            );
        }
    }

    pub fn build_ost_quota_stats(
        x: &TargetStat<QuotaStatsOsd>,
        otel_quota: &OpenTelemetryMetricsQuota,
    ) {
        let TargetStat {
            kind,
            target,
            value,
            ..
        } = x;

        for s in &value.stats {
            let accounting = match value.kind {
                lustre_collector::QuotaKind::Usr => "user",
                lustre_collector::QuotaKind::Grp => "group",
                lustre_collector::QuotaKind::Prj => "project",
            };

            otel_quota.quota_used_inodes.record(
                s.usage.inodes,
                &[
                    KeyValue::new("component", kind.to_string()),
                    KeyValue::new("accounting", accounting),
                    KeyValue::new("target", target.to_string()),
                    KeyValue::new("id", s.id.to_string()),
                ],
            );
            otel_quota.quota_used_kbytes.record(
                s.usage.kbytes,
                &[
                    KeyValue::new("component", kind.to_string()),
                    KeyValue::new("accounting", accounting),
                    KeyValue::new("target", target.to_string()),
                    KeyValue::new("id", s.id.to_string()),
                ],
            );
        }
    }
}

static QUOTA_HARD: Metric = Metric {
    name: "lustre_quota_hard",
    help: "The hard quota for a given component.",
    r#type: MetricType::Gauge,
};

static QUOTA_SOFT: Metric = Metric {
    name: "lustre_quota_soft",
    help: "The soft quota for a given component.",
    r#type: MetricType::Gauge,
};

static QUOTA_GRANTED: Metric = Metric {
    name: "lustre_quota_granted",
    help: "The granted quota for a given component.",
    r#type: MetricType::Gauge,
};

static QUOTA_USED_KBYTES: Metric = Metric {
    name: "lustre_quota_used_kbytes",
    help: "The hard quota for a given component.",
    r#type: MetricType::Gauge,
};

static QUOTA_USED_INODES: Metric = Metric {
    name: "lustre_quota_used_inodes",
    help: "The amount of inodes used by quota.",
    r#type: MetricType::Gauge,
};

pub fn build_quota_stats(
    x: TargetQuotaStat<QuotaStats>,
    stats_map: &mut BTreeMap<&'static str, PrometheusMetric<'static>>,
) {
    let TargetQuotaStat {
        target,
        value,
        pool,
        manager,
        param,
        ..
    } = x;

    for s in value.stats {
        let pool = pool.deref();
        let pool = if pool == "0x0" { "" } else { pool };
        let accounting = match param.deref() {
            "usr" => "user",
            "grp" => "group",
            "prj" => "project",
            _ => param.deref(),
        };
        stats_map
            .get_mut_metric(QUOTA_HARD)
            .render_and_append_instance(
                &PrometheusInstance::new()
                    .with_label("target", target.deref())
                    .with_label("pool", pool)
                    .with_label("accounting", accounting)
                    .with_label("manager", manager.deref())
                    .with_label("id", s.id.to_string().as_str())
                    .with_value(s.limits.hard),
            );

        stats_map
            .get_mut_metric(QUOTA_SOFT)
            .render_and_append_instance(
                &PrometheusInstance::new()
                    .with_label("target", target.deref())
                    .with_label("pool", pool)
                    .with_label("accounting", accounting)
                    .with_label("manager", manager.deref())
                    .with_label("id", s.id.to_string().as_str())
                    .with_value(s.limits.soft),
            );

        stats_map
            .get_mut_metric(QUOTA_GRANTED)
            .render_and_append_instance(
                &PrometheusInstance::new()
                    .with_label("target", target.deref())
                    .with_label("pool", pool)
                    .with_label("accounting", accounting)
                    .with_label("manager", manager.deref())
                    .with_label("id", s.id.to_string().as_str())
                    .with_value(s.limits.granted),
            );
    }
}

pub fn build_ost_quota_stats(
    x: TargetStat<QuotaStatsOsd>,
    stats_map: &mut BTreeMap<&'static str, PrometheusMetric<'static>>,
) {
    let TargetStat {
        kind,
        target,
        value,
        ..
    } = x;

    for s in value.stats {
        let accounting = match value.kind {
            lustre_collector::QuotaKind::Usr => "user",
            lustre_collector::QuotaKind::Grp => "group",
            lustre_collector::QuotaKind::Prj => "project",
        };

        stats_map
            .get_mut_metric(QUOTA_USED_INODES)
            .render_and_append_instance(
                &PrometheusInstance::new()
                    .with_label("component", kind.to_prom_label())
                    .with_label("accounting", accounting)
                    .with_label("target", target.deref())
                    .with_label("id", s.id.to_string().as_str())
                    .with_value(s.usage.inodes),
            );

        stats_map
            .get_mut_metric(QUOTA_USED_KBYTES)
            .render_and_append_instance(
                &PrometheusInstance::new()
                    .with_label("component", kind.to_prom_label())
                    .with_label("accounting", accounting)
                    .with_label("target", target.deref())
                    .with_label("id", s.id.to_string().as_str())
                    .with_value(s.usage.kbytes),
            );
    }
}

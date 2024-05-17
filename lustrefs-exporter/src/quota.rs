// Copyright (c) 2024 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{LabelProm, Metric, StatsMapExt};
use lustre_collector::{QuotaStats, QuotaStatsOsd, TargetQuotaStat, TargetStat};
use prometheus_exporter_base::prelude::*;
use std::{collections::BTreeMap, ops::Deref};

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

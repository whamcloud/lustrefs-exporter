// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{Family, LabelProm, create_labels};
use lustre_collector::{QuotaStats, QuotaStatsOsd, TargetQuotaStat, TargetStat};
use prometheus_client::{metrics::gauge::Gauge, registry::Registry};
use std::{ops::Deref, sync::atomic::AtomicU64};

#[derive(Debug, Default)]
pub struct QuotaMetrics {
    quota_hard: Family<Gauge<u64, AtomicU64>>,
    quota_soft: Family<Gauge<u64, AtomicU64>>,
    quota_granted: Family<Gauge<u64, AtomicU64>>,
    quota_used_kbytes: Family<Gauge<u64, AtomicU64>>,
    quota_used_inodes: Family<Gauge<u64, AtomicU64>>,
}

impl QuotaMetrics {
    pub fn register_metric(&self, registry: &mut Registry) {
        registry.register(
            "lustre_quota_hard",
            "The hard quota for a given component",
            self.quota_hard.clone(),
        );

        registry.register(
            "lustre_quota_soft",
            "The soft quota for a given component",
            self.quota_soft.clone(),
        );

        registry.register(
            "lustre_quota_granted",
            "The granted quota for a given component",
            self.quota_granted.clone(),
        );

        registry.register(
            "lustre_quota_used_kbytes",
            "The hard quota for a given component",
            self.quota_used_kbytes.clone(),
        );

        registry.register(
            "lustre_quota_used_inodes",
            "The amount of inodes used by quota",
            self.quota_used_inodes.clone(),
        );
    }
}

pub fn build_quota_stats(x: &TargetQuotaStat<QuotaStats>, quota: &mut QuotaMetrics) {
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
        let pool = if pool == "0x0" { String::new() } else { pool };
        let accounting = match param.deref() {
            "usr" => "user".to_string(),
            "grp" => "group".to_string(),
            "prj" => "project".to_string(),
            _ => param.to_string(),
        };

        quota
            .quota_hard
            .get_or_create(&create_labels(&[
                ("accounting", accounting.clone()),
                ("id", s.id.to_string()),
                ("manager", manager.to_string()),
                ("pool", pool.clone()),
                ("target", target.to_string()),
            ]))
            .set(s.limits.hard);

        quota
            .quota_soft
            .get_or_create(&create_labels(&[
                ("accounting", accounting.clone()),
                ("id", s.id.to_string()),
                ("manager", manager.to_string()),
                ("pool", pool.clone()),
                ("target", target.to_string()),
            ]))
            .set(s.limits.soft);

        quota
            .quota_granted
            .get_or_create(&create_labels(&[
                ("accounting", accounting),
                ("id", s.id.to_string()),
                ("manager", manager.to_string()),
                ("pool", pool),
                ("target", target.to_string()),
            ]))
            .set(s.limits.granted);
    }
}

pub fn build_ost_quota_stats(x: &TargetStat<QuotaStatsOsd>, quota: &mut QuotaMetrics) {
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

        quota
            .quota_used_inodes
            .get_or_create(&create_labels(&[
                ("accounting", accounting.to_string()),
                ("component", kind.to_prom_label().to_string()),
                ("id", s.id.to_string()),
                ("target", target.to_string()),
            ]))
            .set(s.usage.inodes);

        quota
            .quota_used_kbytes
            .get_or_create(&create_labels(&[
                ("accounting", accounting.to_string()),
                ("component", kind.to_prom_label().to_string()),
                ("id", s.id.to_string()),
                ("target", target.to_string()),
            ]))
            .set(s.usage.kbytes);
    }
}

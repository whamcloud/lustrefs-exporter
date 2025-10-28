// Copyright (c) 2025 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::{
    Family, LabelProm as _,
    llite::build_llite_stats,
    metrics::Metrics,
    quota::{build_ost_quota_stats, build_quota_stats},
    stats::{build_export_stats, build_mds_stats, build_stats},
};
use lustre_collector::{
    BrwStats, ChangeLogUser, ChangelogStat, OssStat, Stat, TargetStat, TargetStats,
};
use prometheus_client::{
    metrics::{counter::Counter, gauge::Gauge},
    registry::Registry,
};
use std::{collections::HashSet, sync::atomic::AtomicU64};

#[derive(Debug, Default)]
pub struct BrwStatsMetrics {
    pub(crate) disk_io_total: Family<Counter<u64>>,
    pub(crate) disk_io_frags: Family<Counter<u64>>,
    pub(crate) disk_io: Family<Counter<u64>>,
    pub(crate) discontiguous_pages_total: Family<Counter<u64>>,
    pub(crate) discontiguous_blocks_total: Family<Counter<u64>>,
    pub(crate) io_time_milliseconds_total: Family<Counter<u64>>,
    pub(crate) pages_per_bulk_rw_total: Family<Counter<u64>>,
    pub(crate) inodes_free: Family<Gauge<u64, AtomicU64>>,
    pub(crate) inodes_maximum: Family<Gauge<u64, AtomicU64>>,
    pub(crate) available_kbytes: Family<Gauge<u64, AtomicU64>>,
    pub(crate) free_kbytes: Family<Gauge<u64, AtomicU64>>,
    pub(crate) capacity_kbytes: Family<Gauge<u64, AtomicU64>>,
    pub(crate) exports_total: Family<Gauge<u64, AtomicU64>>,
    pub(crate) exports_dirty_total: Family<Gauge<u64, AtomicU64>>,
    pub(crate) exports_granted_total: Family<Gauge<u64, AtomicU64>>,
    pub(crate) exports_pending_total: Family<Gauge<u64, AtomicU64>>,
    pub(crate) lock_contended_total: Family<Gauge<u64, AtomicU64>>,
    pub(crate) lock_contention_seconds_total: Family<Gauge<u64, AtomicU64>>,
    pub(crate) connected_clients: Family<Gauge<u64, AtomicU64>>,
    pub(crate) lock_count_total: Family<Gauge<u64, AtomicU64>>,
    pub(crate) lock_timeout_total: Family<Counter<u64>>,
    pub(crate) block_maps_msec_total: Family<Counter<u64>>,
    pub(crate) recovery_status: Family<Gauge<u64, AtomicU64>>,
    pub(crate) recovery_status_completed_clients: Family<Gauge<u64, AtomicU64>>,
    pub(crate) recovery_status_connected_clients: Family<Gauge<u64, AtomicU64>>,
    pub(crate) recovery_status_evicted_clients: Family<Gauge<u64, AtomicU64>>,
    pub(crate) recovery_status_duration_seconds: Family<Gauge<u64, AtomicU64>>,
    pub(crate) recovery_status_time_remaining_seconds: Family<Gauge<u64, AtomicU64>>,
    pub(crate) recovery_status_total_clients: Family<Gauge<u64, AtomicU64>>,
    pub(crate) ost_stats: Family<Gauge<u64, AtomicU64>>,
    pub(crate) ost_io_stats: Family<Gauge<u64, AtomicU64>>,
    pub(crate) ost_create_stats: Family<Gauge<u64, AtomicU64>>,
    pub(crate) changelog_current_index: Family<Gauge<u64, AtomicU64>>,
    pub(crate) changelog_user_index: Family<Gauge<u64, AtomicU64>>,
    pub(crate) changelog_user_idle_sec: Family<Gauge<u64, AtomicU64>>,
}

impl BrwStatsMetrics {
    pub fn register_metric(&self, registry: &mut Registry) {
        registry.register_without_auto_suffix(
            "lustre_disk_io_total",
            "Total number of operations the filesystem has performed for the given size. 'size' label represents 'Disk I/O size', the size of each I/O operation",
            self.disk_io_total.clone()
        );

        registry.register_without_auto_suffix(
            "lustre_dio_frags",
            "Current disk IO fragmentation for the given size. 'size' label represents 'Disk fragmented I/Os', the number of I/Os that were not written entirely sequentially",
            self.disk_io_frags.clone()
        );

        registry.register_without_auto_suffix(
            "lustre_disk_io",
            "Current number of I/O operations that are processing during the snapshot. 'size' label represents 'Disk I/Os in flight', the number of disk I/Os currently pending",
            self.disk_io.clone()
        );

        registry.register_without_auto_suffix(
            "lustre_discontiguous_pages_total",
            "Total number of logical discontinuities per RPC. 'size' label represents 'Discontiguous pages', the number of discontinuities in the logical file offset of each page in a single RPC",
            self.discontiguous_pages_total.clone()
        );

        registry.register(
            "lustre_discontiguous_blocks",
            "'size' label represents 'Discontiguous blocks', the number of discontinuities in the physical block allocation in the file system for a single RPC",
    self.discontiguous_blocks_total.clone()
        );

        registry.register(
            "lustre_io_time_milliseconds",
            "Total time in milliseconds the filesystem has spent processing various object sizes. 'size' label represents 'I/O time (1/1000s)', the amount of time for each I/O operation to complete",
    self.io_time_milliseconds_total.clone()
        );

        registry.register(
            "lustre_pages_per_bulk_rw",
            "Total number of pages per block RPC. 'size' label represents 'Pages per bulk r/w', the number of pages per RPC request",
    self.pages_per_bulk_rw_total.clone()
        );

        registry.register(
            "lustre_inodes_free",
            "The number of inodes (objects) available",
            self.inodes_free.clone(),
        );

        registry.register(
            "lustre_inodes_maximum",
            "The maximum number of inodes (objects) the filesystem can hold",
            self.inodes_maximum.clone(),
        );

        registry.register(
            "lustre_available_kilobytes",
            "Number of kilobytes readily available in the pool",
            self.available_kbytes.clone(),
        );

        registry.register(
            "lustre_free_kilobytes",
            "Number of kilobytes allocated to the pool",
            self.free_kbytes.clone(),
        );

        registry.register(
            "lustre_capacity_kilobytes",
            "Capacity of the pool in kilobytes",
            self.capacity_kbytes.clone(),
        );

        registry.register_without_auto_suffix(
            "lustre_exports_total",
            "Total number of times the pool has been exported",
            self.exports_total.clone(),
        );

        registry.register_without_auto_suffix(
            "lustre_exports_dirty_total",
            "Total number of exports that have been marked dirty",
            self.exports_dirty_total.clone(),
        );

        registry.register(
            "lustre_exports_granted_total",
            "Total number of exports that have been marked granted",
            self.exports_granted_total.clone(),
        );

        registry.register(
            "lustre_exports_pending_total",
            "Total number of exports that have been marked pending",
            self.exports_pending_total.clone(),
        );

        registry.register(
            "lustre_lock_contended_total",
            "Number of contended locks",
            self.lock_contended_total.clone(),
        );

        registry.register_without_auto_suffix(
            "lustre_lock_contention_seconds_total",
            "Time in seconds during which locks were contended",
            self.lock_contention_seconds_total.clone(),
        );

        registry.register(
            "lustre_connected_clients",
            "Number of connected clients",
            self.connected_clients.clone(),
        );

        registry.register(
            "lustre_lock_count_total",
            "Number of locks",
            self.lock_count_total.clone(),
        );

        registry.register(
            "lustre_lock_timeout",
            "Number of lock timeouts",
            self.lock_timeout_total.clone(),
        );

        registry.register(
            "lustre_block_maps_milliseconds",
            "Number of block maps in milliseconds",
            self.block_maps_msec_total.clone(),
        );

        registry.register(
            "recovery_status",
            "Gives the recovery status off a target. 0=Complete 1=Inactive 2=Waiting 3=WaitingForClients 4=Recovering 5=Unknown",
            self.recovery_status.clone()
        );

        registry.register(
            "recovery_status_completed_clients",
            "Gives the count of clients that complete the recovery on a target",
            self.recovery_status_completed_clients.clone(),
        );

        registry.register(
            "recovery_status_connected_clients",
            "Gives the count of clients connected to a target",
            self.recovery_status_connected_clients.clone(),
        );

        registry.register(
            "recovery_status_evicted_clients",
            "Gives the count of clients evicted from a target",
            self.recovery_status_evicted_clients.clone(),
        );

        registry.register(
            "recovery_status_duration_seconds",
            "Gives the total duration in seconds of the recovery on a target",
            self.recovery_status_duration_seconds.clone(),
        );

        registry.register(
            "recovery_status_time_remaining_seconds",
            "Gives the estimated time remaining in seconds of the recovery on a target",
            self.recovery_status_time_remaining_seconds.clone(),
        );

        registry.register(
            "recovery_status_total_clients",
            "Gives the total number of clients involved in the recovery on a target",
            self.recovery_status_total_clients.clone(),
        );

        registry.register(
            "lustre_oss_ost_stats",
            "OSS ost stats",
            self.ost_stats.clone(),
        );

        registry.register(
            "lustre_oss_ost_io_stats",
            "OSS ost_io stats",
            self.ost_io_stats.clone(),
        );

        registry.register(
            "lustre_oss_ost_create_stats",
            "OSS ost_create stats",
            self.ost_create_stats.clone(),
        );

        registry.register(
            "lustre_changelog_current_index",
            "current changelog index",
            self.changelog_current_index.clone(),
        );

        registry.register(
            "lustre_changelog_user_index",
            "current, maximum changelog index per registered changelog user",
            self.changelog_user_index.clone(),
        );

        registry.register(
            "lustre_changelog_user_idle_sec",
            "current changelog user idle seconds",
            self.changelog_user_idle_sec.clone(),
        );
    }
}

fn build_brw_stats(
    x: &TargetStat<Vec<BrwStats>>,
    brw: &mut BrwStatsMetrics,
    set: &mut HashSet<(String, String, String, String)>,
) {
    let TargetStat {
        kind,
        target,
        value,
        ..
    } = x;

    for x in value {
        let BrwStats { name, buckets, .. } = x;

        for b in buckets {
            let size = b.name.to_string();

            let labels = vec![
                ("component", kind.to_prom_label().to_string()),
                ("operation", "read".into()),
                ("size", size.clone()),
                ("target", target.to_string()),
            ];

            let write_labels = vec![
                ("component", kind.to_prom_label().to_string()),
                ("operation", "write".into()),
                ("size", size.clone()),
                ("target", target.to_string()),
            ];

            if set.insert((
                kind.to_prom_label().to_string(),
                target.to_string(),
                size.clone(),
                name.clone(),
            )) {
                match name.as_str() {
                    "disk_iosize" => {
                        brw.disk_io_total.get_or_create(&labels).inc_by(b.read);

                        brw.disk_io_total
                            .get_or_create(&write_labels)
                            .inc_by(b.write);
                    }
                    "rpc_hist" => {
                        brw.disk_io.get_or_create(&labels).inc_by(b.read);

                        brw.disk_io.get_or_create(&write_labels).inc_by(b.write);
                    }
                    "pages" => {
                        brw.pages_per_bulk_rw_total
                            .get_or_create(&labels)
                            .inc_by(b.read);

                        brw.pages_per_bulk_rw_total
                            .get_or_create(&write_labels)
                            .inc_by(b.write);
                    }
                    "discont_pages" => {
                        brw.discontiguous_pages_total
                            .get_or_create(&labels)
                            .inc_by(b.read);

                        brw.discontiguous_pages_total
                            .get_or_create(&write_labels)
                            .inc_by(b.write);
                    }
                    "dio_frags" => {
                        brw.disk_io_frags.get_or_create(&labels).inc_by(b.read);

                        brw.disk_io_frags
                            .get_or_create(&write_labels)
                            .inc_by(b.write);
                    }
                    "discont_blocks" => {
                        brw.discontiguous_blocks_total
                            .get_or_create(&labels)
                            .inc_by(b.read);

                        brw.discontiguous_blocks_total
                            .get_or_create(&write_labels)
                            .inc_by(b.write);
                    }
                    "io_time" => {
                        brw.io_time_milliseconds_total
                            .get_or_create(&labels)
                            .inc_by(b.read);

                        brw.io_time_milliseconds_total
                            .get_or_create(&write_labels)
                            .inc_by(b.write);
                    }
                    "block_maps_msec" => {
                        brw.block_maps_msec_total
                            .get_or_create(&labels)
                            .inc_by(b.read);

                        brw.block_maps_msec_total
                            .get_or_create(&write_labels)
                            .inc_by(b.write);
                    }
                    _ => {}
                }
            }
        }
    }
}

fn build_oss_stats(x: &OssStat, brw: &mut BrwStatsMetrics) {
    let OssStat { param, stats } = x;

    for x in stats {
        let Stat {
            name,
            units,
            samples,
            ..
        } = x;

        let labels = vec![
            ("operation", name.to_string()),
            ("units", units.to_string()),
        ];

        match param.0.as_str() {
            "ost" => brw.ost_stats.get_or_create(&labels).set(*samples),
            "ost_io" => brw.ost_io_stats.get_or_create(&labels).set(*samples),
            "ost_create" => brw.ost_create_stats.get_or_create(&labels).set(*samples),
            _ => 0,
        };
    }
}

fn build_changelog_stats(x: &TargetStat<ChangelogStat>, brw: &mut BrwStatsMetrics) {
    let TargetStat { target, value, .. } = x;

    let ChangelogStat {
        current_index,
        users,
    } = value;

    brw.changelog_current_index
        .get_or_create(&vec![("target", target.to_string())])
        .set(*current_index);

    for user in users {
        let ChangeLogUser {
            user,
            index,
            idle_secs,
        } = user;

        brw.changelog_user_index
            .get_or_create(&vec![
                ("target", target.to_string()),
                ("user", user.to_string()),
            ])
            .set(*index);

        brw.changelog_user_idle_sec
            .get_or_create(&vec![("user", user.to_string())])
            .set(*idle_secs);
    }
}

pub fn build_target_stats(
    x: &TargetStats,
    metrics: &mut Metrics,
    set: &mut HashSet<(String, String, String, String)>,
) {
    match x {
        TargetStats::Stats(x) => {
            build_stats(x, &mut metrics.stats);
        }
        TargetStats::BrwStats(x) => {
            build_brw_stats(x, &mut metrics.brw, set);
        }
        TargetStats::FilesFree(x) => {
            metrics
                .brw
                .inodes_free
                .get_or_create(&vec![
                    ("component", x.kind.to_prom_label().to_string()),
                    ("target", x.target.to_string()),
                ])
                .set(x.value);
        }
        TargetStats::FilesTotal(x) => {
            metrics
                .brw
                .inodes_maximum
                .get_or_create(&vec![
                    ("component", x.kind.to_prom_label().to_string()),
                    ("target", x.target.to_string()),
                ])
                .set(x.value);
        }
        TargetStats::KBytesAvail(x) => {
            metrics
                .brw
                .available_kbytes
                .get_or_create(&vec![
                    ("component", x.kind.to_prom_label().to_string()),
                    ("target", x.target.to_string()),
                ])
                .set(x.value);
        }
        TargetStats::KBytesFree(x) => {
            metrics
                .brw
                .free_kbytes
                .get_or_create(&vec![
                    ("component", x.kind.to_prom_label().to_string()),
                    ("target", x.target.to_string()),
                ])
                .set(x.value);
        }
        TargetStats::KBytesTotal(x) => {
            metrics
                .brw
                .capacity_kbytes
                .get_or_create(&vec![
                    ("component", x.kind.to_prom_label().to_string()),
                    ("target", x.target.to_string()),
                ])
                .set(x.value);
        }
        TargetStats::NumExports(x) => {
            metrics
                .brw
                .exports_total
                .get_or_create(&vec![
                    ("component", x.kind.to_prom_label().to_string()),
                    ("target", x.target.to_string()),
                ])
                .set(x.value);
        }
        TargetStats::TotDirty(x) => {
            metrics
                .brw
                .exports_dirty_total
                .get_or_create(&vec![
                    ("component", x.kind.to_prom_label().to_string()),
                    ("target", x.target.to_string()),
                ])
                .set(x.value);
        }
        TargetStats::TotGranted(x) => {
            metrics
                .brw
                .exports_granted_total
                .get_or_create(&vec![
                    ("component", x.kind.to_prom_label().to_string()),
                    ("target", x.target.to_string()),
                ])
                .set(x.value);
        }
        TargetStats::TotPending(x) => {
            metrics
                .brw
                .exports_pending_total
                .get_or_create(&vec![
                    ("component", x.kind.to_prom_label().to_string()),
                    ("target", x.target.to_string()),
                ])
                .set(x.value);
        }
        TargetStats::ContendedLocks(x) => {
            metrics
                .brw
                .lock_contended_total
                .get_or_create(&vec![
                    ("component", x.kind.to_prom_label().to_string()),
                    ("target", x.target.to_string()),
                ])
                .set(x.value);
        }
        TargetStats::ContentionSeconds(x) => {
            metrics
                .brw
                .lock_contention_seconds_total
                .get_or_create(&vec![
                    ("component", x.kind.to_prom_label().to_string()),
                    ("target", x.target.to_string()),
                ])
                .set(x.value);
        }
        TargetStats::ConnectedClients(x) => {
            metrics
                .brw
                .connected_clients
                .get_or_create(&vec![
                    ("component", x.kind.to_prom_label().to_string()),
                    ("target", x.target.to_string()),
                ])
                .set(x.value);
        }
        TargetStats::LockCount(x) => {
            metrics
                .brw
                .lock_count_total
                .get_or_create(&vec![
                    ("component", x.kind.to_prom_label().to_string()),
                    ("target", x.target.to_string()),
                ])
                .set(x.value);
        }
        TargetStats::LockTimeouts(x) => {
            metrics
                .brw
                .lock_timeout_total
                .get_or_create(&vec![
                    ("component", x.kind.to_prom_label().to_string()),
                    ("target", x.target.to_string()),
                ])
                .inc_by(x.value);
        }
        TargetStats::Llite(x) => build_llite_stats(x, &mut metrics.llite),
        TargetStats::RecoveryStatus(x) => {
            metrics
                .brw
                .recovery_status
                .get_or_create(&vec![
                    ("kind", x.kind.to_string()),
                    ("target", x.target.to_string()),
                ])
                .set(x.value as u64);
        }
        TargetStats::RecoveryCompletedClients(x) => {
            metrics
                .brw
                .recovery_status_completed_clients
                .get_or_create(&vec![
                    ("kind", x.kind.to_string()),
                    ("target", x.target.to_string()),
                ])
                .set(x.value);
        }
        TargetStats::RecoveryConnectedClients(x) => {
            metrics
                .brw
                .recovery_status_connected_clients
                .get_or_create(&vec![
                    ("kind", x.kind.to_string()),
                    ("target", x.target.to_string()),
                ])
                .set(x.value);
        }
        TargetStats::RecoveryEvictedClients(x) => {
            metrics
                .brw
                .recovery_status_evicted_clients
                .get_or_create(&vec![
                    ("kind", x.kind.to_string()),
                    ("target", x.target.to_string()),
                ])
                .set(x.value);
        }
        TargetStats::RecoveryDuration(x) => {
            metrics
                .brw
                .recovery_status_duration_seconds
                .get_or_create(&vec![
                    ("kind", x.kind.to_string()),
                    ("target", x.target.to_string()),
                ])
                .set(x.value);
        }
        TargetStats::RecoveryTimeRemaining(x) => {
            metrics
                .brw
                .recovery_status_time_remaining_seconds
                .get_or_create(&vec![
                    ("kind", x.kind.to_string()),
                    ("target", x.target.to_string()),
                ])
                .set(x.value);
        }
        TargetStats::RecoveryTotalClients(x) => {
            metrics
                .brw
                .recovery_status_total_clients
                .get_or_create(&vec![
                    ("kind", x.kind.to_string()),
                    ("target", x.target.to_string()),
                ])
                .set(x.value);
        }
        TargetStats::ExportStats(x) => {
            build_export_stats(x, &mut metrics.export);
        }
        TargetStats::QuotaStats(x) => {
            build_quota_stats(x, &mut metrics.quota);
        }
        TargetStats::QuotaStatsOsd(x) => {
            build_ost_quota_stats(x, &mut metrics.quota);
        }
        TargetStats::Oss(x) => build_oss_stats(x, &mut metrics.brw),
        TargetStats::Changelog(x) => build_changelog_stats(x, &mut metrics.brw),
        TargetStats::Mds(x) => build_mds_stats(x, &mut metrics.mds),
        _ => {}
    };
}

// Copyright (c) 2024 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.
pub mod opentelemetry {
    use std::collections::HashSet;

    use lustre_collector::{
        BrwStats, ChangeLogUser, ChangelogStat, OssStat, Stat, TargetStat, TargetStats,
    };
    use opentelemetry::{
        metrics::{Counter, Gauge, Meter},
        KeyValue,
    };

    use crate::llite::opentelemetry::build_llite_stats;
    use crate::quota::opentelemetry::{build_ost_quota_stats, build_quota_stats};
    use crate::stats::opentelemetry::{build_export_stats, build_mds_stats, build_stats};
    use crate::{openmetrics::OpenTelemetryMetrics, LabelProm as _};

    #[derive(Debug)]
    pub struct OpenTelemetryMetricsBrw {
        pub disk_io_total: Counter<u64>,
        pub disk_io_frags: Gauge<u64>,
        pub disk_io: Gauge<u64>,
        pub discontiguous_pages_total: Counter<u64>,
        pub discontiguous_blocks_total: Counter<u64>,
        pub io_time_milliseconds_total: Counter<u64>,
        pub pages_per_bulk_rw_total: Counter<u64>,
        pub inodes_free: Gauge<u64>,
        pub inodes_maximum: Gauge<u64>,
        pub available_kbytes: Gauge<u64>,
        pub free_kbytes: Gauge<u64>,
        pub capacity_kbytes: Gauge<u64>,
        pub exports_total: Counter<u64>,
        pub exports_dirty_total: Counter<u64>,
        pub exports_granted_total: Counter<u64>,
        pub exports_pending_total: Counter<u64>,
        pub lock_contended_total: Counter<u64>,
        pub lock_contention_seconds_total: Counter<u64>,
        pub connected_clients: Gauge<u64>,
        pub lock_count_total: Counter<u64>,
        pub lock_timeout_total: Counter<u64>,
        pub block_maps_msec_total: Counter<u64>,
        pub recovery_status: Gauge<u64>,
        pub recovery_status_completed_clients: Gauge<u64>,
        pub recovery_status_connected_clients: Gauge<u64>,
        pub recovery_status_evicted_clients: Gauge<u64>,
        pub ost_stats: Gauge<u64>,
        pub ost_io_stats: Gauge<u64>,
        pub ost_create_stats: Gauge<u64>,
        pub changelog_current_index: Gauge<u64>,
        pub changelog_user_index: Gauge<u64>,
        pub changelog_user_idle_sec: Gauge<u64>,
    }

    impl OpenTelemetryMetricsBrw {
        pub fn new(meter: &Meter) -> Self {
            OpenTelemetryMetricsBrw {
                disk_io_total: meter
                    .u64_counter("lustre_disk_io_total")
                    .with_description("Total number of operations the filesystem has performed for the given size. 'size' label represents 'Disk I/O size', the size of each I/O operation")
                    .build(),
                disk_io_frags: meter
                    .u64_gauge("lustre_dio_frags")
                    .with_description("Current disk IO fragmentation for the given size. 'size' label represents 'Disk fragmented I/Os', the number of I/Os that were not written entirely sequentially.")
                    .build(),
                disk_io: meter
                    .u64_gauge("lustre_disk_io")
                    .with_description("Current number of I/O operations that are processing during the snapshot. 'size' label represents 'Disk I/Os in flight', the number of disk I/Os currently pending.")
                    .build(),
                discontiguous_pages_total: meter
                    .u64_counter("lustre_discontiguous_pages_total")
                    .with_description("Total number of logical discontinuities per RPC. 'size' label represents 'Discontiguous pages', the number of discontinuities in the logical file offset of each page in a single RPC.")
                    .build(),
                discontiguous_blocks_total: meter
                    .u64_counter("lustre_discontiguous_blocks_total")
                    .with_description("'size' label represents 'Discontiguous blocks', the number of discontinuities in the physical block allocation in the file system for a single RPC")
                    .build(),
                io_time_milliseconds_total: meter
                    .u64_counter("lustre_io_time_milliseconds_total")
                    .with_description("Total time in milliseconds the filesystem has spent processing various object sizes. 'size' label represents 'I/O time (1/1000s)', the amount of time for each I/O operation to complete.")
                    .build(),
                pages_per_bulk_rw_total: meter
                    .u64_counter("lustre_pages_per_bulk_rw_total")
                    .with_description("Total number of pages per block RPC. 'size' label represents 'Pages per bulk r/w', the number of pages per RPC request")
                    .build(),
                inodes_free: meter
                    .u64_gauge("lustre_inodes_free")
                    .with_description("The number of inodes (objects) available")
                    .build(),
                inodes_maximum: meter
                    .u64_gauge("lustre_inodes_maximum")
                    .with_description("The maximum number of inodes (objects) the filesystem can hold")
                    .build(),
                available_kbytes: meter
                    .u64_gauge("lustre_available_kilobytes")
                    .with_description("Number of kilobytes readily available in the pool")
                    .build(),
                free_kbytes: meter
                    .u64_gauge("lustre_free_kilobytes")
                    .with_description("Number of kilobytes allocated to the pool")
                    .build(),
                capacity_kbytes: meter
                    .u64_gauge("lustre_capacity_kilobytes")
                    .with_description("Capacity of the pool in kilobytes")
                    .build(),
                exports_total: meter
                    .u64_counter("lustre_exports_total")
                    .with_description("Total number of times the pool has been exported")
                    .build(),
                exports_dirty_total: meter
                    .u64_counter("lustre_exports_dirty_total")
                    .with_description("Total number of exports that have been marked dirty")
                    .build(),
                exports_granted_total: meter
                    .u64_counter("lustre_exports_granted_total")
                    .with_description("Total number of exports that have been marked granted")
                    .build(),
                exports_pending_total: meter
                    .u64_counter("lustre_exports_pending_total")
                    .with_description("Total number of exports that have been marked pending")
                    .build(),
                lock_contended_total: meter
                    .u64_counter("lustre_lock_contended_total")
                    .with_description("Number of contended locks")
                    .build(),
                lock_contention_seconds_total: meter
                    .u64_counter("lustre_lock_contention_seconds_total")
                    .with_description("Time in seconds during which locks were contended")
                    .build(),
                connected_clients: meter
                    .u64_gauge("lustre_connected_clients")
                    .with_description("Number of connected clients")
                    .build(),
                lock_count_total: meter
                    .u64_counter("lustre_lock_count_total")
                    .with_description("Number of locks")
                    .build(),
                lock_timeout_total: meter
                    .u64_counter("lustre_lock_timeout_total")
                    .with_description("Number of lock timeouts")
                    .build(),
                block_maps_msec_total: meter
                    .u64_counter("lustre_block_maps_milliseconds_total")
                    .with_description("Number of block maps in milliseconds")
                    .build(),
                recovery_status: meter
                    .u64_gauge("recovery_status")
                    .with_description("Gives the recovery status off a target. 0=Complete 1=Inactive 2=Waiting 3=WaitingForClients 4=Recovering 5=Unknown")
                    .build(),
                recovery_status_completed_clients: meter
                    .u64_gauge("recovery_status_completed_clients")
                    .with_description("Gives the count of clients that complete the recovery on a target.")
                    .build(),
                recovery_status_connected_clients: meter
                    .u64_gauge("recovery_status_connected_clients")
                    .with_description("Gives the count of clients connected to a target.")
                    .build(),
                recovery_status_evicted_clients: meter
                    .u64_gauge("recovery_status_evicted_clients")
                    .with_description("Gives the count of clients evicted from a target.")
                    .build(),
                ost_stats: meter
                    .u64_gauge("lustre_oss_ost_stats")
                    .with_description("OSS ost stats")
                    .build(),
                ost_io_stats: meter
                    .u64_gauge("lustre_oss_ost_io_stats")
                    .with_description("OSS ost_io stats")
                    .build(),
                ost_create_stats: meter
                    .u64_gauge("lustre_oss_ost_create_stats")
                    .with_description("OSS ost_create stats")
                    .build(),
                changelog_current_index: meter
                    .u64_gauge("lustre_changelog_current_index")
                    .with_description("current changelog index.")
                    .build(),
                changelog_user_index: meter
                    .u64_gauge("lustre_changelog_user_index")
                    .with_description("current, maximum changelog index per registered changelog user.")
                    .build(),
                changelog_user_idle_sec: meter
                    .u64_gauge("lustre_changelog_user_idle_sec")
                    .with_description("current changelog user idle seconds.")
                    .build(),
            }
        }
    }

    fn build_brw_stats(
        x: &TargetStat<Vec<BrwStats>>,
        otel_brw: &OpenTelemetryMetricsBrw,
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
                let labels = &[
                    KeyValue::new("component", kind.to_prom_label().to_string()),
                    KeyValue::new("target", target.to_string()),
                    KeyValue::new("size", size.clone()),
                    KeyValue::new("operation", "read"),
                ];
                let write_labels = &[
                    KeyValue::new("component", kind.to_prom_label().to_string()),
                    KeyValue::new("target", target.to_string()),
                    KeyValue::new("size", size.clone()),
                    KeyValue::new("operation", "write"),
                ];

                if set.insert((
                    kind.to_prom_label().to_string(),
                    target.to_string(),
                    size.clone(),
                    name.clone(),
                )) {
                    match name.as_str() {
                        "disk_iosize" => {
                            otel_brw.disk_io_total.add(b.read, labels);
                            otel_brw.disk_io_total.add(b.write, write_labels);
                        }
                        "rpc_hist" => {
                            otel_brw.disk_io.record(b.read, labels);
                            otel_brw.disk_io.record(b.write, write_labels);
                        }
                        "pages" => {
                            otel_brw.pages_per_bulk_rw_total.add(b.read, labels);
                            otel_brw.pages_per_bulk_rw_total.add(b.write, write_labels);
                        }
                        "discont_pages" => {
                            otel_brw.discontiguous_pages_total.add(b.read, labels);
                            otel_brw
                                .discontiguous_pages_total
                                .add(b.write, write_labels);
                        }
                        "dio_frags" => {
                            otel_brw.disk_io_frags.record(b.read, labels);
                            otel_brw.disk_io_frags.record(b.write, write_labels);
                        }
                        "discont_blocks" => {
                            otel_brw.discontiguous_blocks_total.add(b.read, labels);
                            otel_brw
                                .discontiguous_blocks_total
                                .add(b.write, write_labels);
                        }
                        "io_time" => {
                            otel_brw.io_time_milliseconds_total.add(b.read, labels);
                            otel_brw
                                .io_time_milliseconds_total
                                .add(b.write, write_labels);
                        }
                        "block_maps_msec" => {
                            otel_brw.block_maps_msec_total.add(b.read, labels);
                            otel_brw.block_maps_msec_total.add(b.write, write_labels);
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn build_oss_stats(x: &OssStat, otel_brw: &OpenTelemetryMetricsBrw) {
        let OssStat { param, stats } = x;

        for x in stats {
            let Stat {
                name,
                units,
                samples,
                ..
            } = x;

            let labels = &[
                KeyValue::new("operation", name.to_string()),
                KeyValue::new("units", units.to_string()),
            ];

            match param.0.as_str() {
                "ost" => otel_brw.ost_stats.record(*samples, labels),
                "ost_io" => otel_brw.ost_io_stats.record(*samples, labels),
                "ost_create" => otel_brw.ost_create_stats.record(*samples, labels),
                _ => {}
            }
        }
    }

    fn build_changelog_stats(x: &TargetStat<ChangelogStat>, otel_brw: &OpenTelemetryMetricsBrw) {
        let TargetStat { target, value, .. } = x;

        let ChangelogStat {
            current_index,
            users,
        } = value;

        otel_brw.changelog_current_index.record(
            *current_index,
            &[KeyValue::new("target", target.to_string())],
        );

        for user in users {
            let ChangeLogUser {
                user,
                index,
                idle_secs,
            } = user;

            otel_brw.changelog_user_index.record(
                *index,
                &[
                    KeyValue::new("user", user.to_string()),
                    KeyValue::new("target", target.to_string()),
                ],
            );

            otel_brw
                .changelog_user_idle_sec
                .record(*idle_secs, &[KeyValue::new("user", user.to_string())]);
        }
    }

    pub fn build_target_stats(
        x: &TargetStats,
        otel: &OpenTelemetryMetrics,
        set: &mut HashSet<(String, String, String, String)>,
    ) {
        match x {
            TargetStats::Stats(x) => {
                build_stats(x, &otel.stats);
            }
            TargetStats::BrwStats(x) => {
                build_brw_stats(x, &otel.brw, set);
            }
            TargetStats::FilesFree(x) => {
                otel.brw.inodes_free.record(
                    x.value,
                    &[
                        KeyValue::new("component", x.kind.to_prom_label().to_string()),
                        KeyValue::new("target", x.target.to_string()),
                    ],
                );
            }
            TargetStats::FilesTotal(x) => {
                otel.brw.inodes_maximum.record(
                    x.value,
                    &[
                        KeyValue::new("component", x.kind.to_prom_label().to_string()),
                        KeyValue::new("target", x.target.to_string()),
                    ],
                );
            }
            TargetStats::KBytesAvail(x) => {
                otel.brw.available_kbytes.record(
                    x.value,
                    &[
                        KeyValue::new("component", x.kind.to_prom_label().to_string()),
                        KeyValue::new("target", x.target.to_string()),
                    ],
                );
            }
            TargetStats::KBytesFree(x) => {
                otel.brw.free_kbytes.record(
                    x.value,
                    &[
                        KeyValue::new("component", x.kind.to_prom_label().to_string()),
                        KeyValue::new("target", x.target.to_string()),
                    ],
                );
            }
            TargetStats::KBytesTotal(x) => {
                otel.brw.capacity_kbytes.record(
                    x.value,
                    &[
                        KeyValue::new("component", x.kind.to_prom_label().to_string()),
                        KeyValue::new("target", x.target.to_string()),
                    ],
                );
            }
            TargetStats::NumExports(x) => {
                otel.brw.exports_total.add(
                    x.value,
                    &[
                        KeyValue::new("component", x.kind.to_prom_label().to_string()),
                        KeyValue::new("target", x.target.to_string()),
                    ],
                );
            }
            TargetStats::TotDirty(x) => {
                otel.brw.exports_dirty_total.add(
                    x.value,
                    &[
                        KeyValue::new("component", x.kind.to_prom_label().to_string()),
                        KeyValue::new("target", x.target.to_string()),
                    ],
                );
            }
            TargetStats::TotGranted(x) => {
                otel.brw.exports_granted_total.add(
                    x.value,
                    &[
                        KeyValue::new("component", x.kind.to_prom_label().to_string()),
                        KeyValue::new("target", x.target.to_string()),
                    ],
                );
            }
            TargetStats::TotPending(x) => {
                otel.brw.exports_pending_total.add(
                    x.value,
                    &[
                        KeyValue::new("component", x.kind.to_prom_label().to_string()),
                        KeyValue::new("target", x.target.to_string()),
                    ],
                );
            }
            TargetStats::ContendedLocks(x) => {
                otel.brw.lock_contended_total.add(
                    x.value,
                    &[
                        KeyValue::new("component", x.kind.to_prom_label().to_string()),
                        KeyValue::new("target", x.target.to_string()),
                    ],
                );
            }
            TargetStats::ContentionSeconds(x) => {
                otel.brw.lock_contention_seconds_total.add(
                    x.value,
                    &[
                        KeyValue::new("component", x.kind.to_prom_label().to_string()),
                        KeyValue::new("target", x.target.to_string()),
                    ],
                );
            }
            TargetStats::ConnectedClients(x) => {
                otel.brw.connected_clients.record(
                    x.value,
                    &[
                        KeyValue::new("component", x.kind.to_prom_label().to_string()),
                        KeyValue::new("target", x.target.to_string()),
                    ],
                );
            }
            TargetStats::LockCount(x) => {
                otel.brw.lock_count_total.add(
                    x.value,
                    &[
                        KeyValue::new("component", x.kind.to_prom_label().to_string()),
                        KeyValue::new("target", x.target.to_string()),
                    ],
                );
            }
            TargetStats::LockTimeouts(x) => {
                otel.brw.lock_timeout_total.add(
                    x.value,
                    &[
                        KeyValue::new("component", x.kind.to_prom_label().to_string()),
                        KeyValue::new("target", x.target.to_string()),
                    ],
                );
            }
            TargetStats::Llite(x) => build_llite_stats(x, &otel.llite),
            TargetStats::RecoveryStatus(x) => {
                otel.brw.recovery_status.record(
                    x.value as u64,
                    &[
                        KeyValue::new("target", x.target.to_string()),
                        KeyValue::new("kind", x.kind.to_string()),
                    ],
                );
            }
            TargetStats::RecoveryCompletedClients(x) => {
                otel.brw.recovery_status_completed_clients.record(
                    x.value,
                    &[
                        KeyValue::new("target", x.target.to_string()),
                        KeyValue::new("kind", x.kind.to_string()),
                    ],
                );
            }
            TargetStats::RecoveryConnectedClients(x) => {
                otel.brw.recovery_status_connected_clients.record(
                    x.value,
                    &[
                        KeyValue::new("target", x.target.to_string()),
                        KeyValue::new("kind", x.kind.to_string()),
                    ],
                );
            }
            TargetStats::RecoveryEvictedClients(x) => {
                otel.brw.recovery_status_evicted_clients.record(
                    x.value,
                    &[
                        KeyValue::new("target", x.target.to_string()),
                        KeyValue::new("kind", x.kind.to_string()),
                    ],
                );
            }
            TargetStats::ExportStats(x) => {
                build_export_stats(x, &otel.export);
            }
            TargetStats::QuotaStats(x) => {
                build_quota_stats(x, &otel.quota);
            }
            TargetStats::QuotaStatsOsd(x) => {
                build_ost_quota_stats(x, &otel.quota);
            }
            TargetStats::Oss(x) => build_oss_stats(x, &otel.brw),
            TargetStats::Changelog(x) => build_changelog_stats(x, &otel.brw),
            TargetStats::Mds(x) => build_mds_stats(x, &otel.mds),
            _ => {}
        };
    }
}

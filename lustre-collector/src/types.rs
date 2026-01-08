// Copyright (c) 2024 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

use crate::LustreCollectorError;
use std::{fmt, ops::Deref, time::Duration};

#[derive(Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
/// The hostname cooresponding to these stats.
pub struct Host(pub String);

impl Deref for Host {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
/// The Lustre target cooresponding to these stats.
pub struct Target(pub String);

impl Deref for Target {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
/// The name of the stat.
pub struct Param(pub String);

impl Deref for Param {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct ReqsStat {
    pub samples: i64,
    pub unit: String,
}

#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct BytesStat {
    pub samples: i64,
    pub unit: String,
    pub min: i64,
    pub max: i64,
    pub sum: i64,
}

#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct ExportStats {
    pub nid: String,
    pub stats: Vec<Stat>,
}

/// Used to represent an unsigned timestamp in Lustre.
///
/// Only use this field when you are sure that the timestamp is unsigned.
#[derive(PartialEq, Eq, Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct UnsignedLustreTimestamp(pub i64);

impl From<UnsignedLustreTimestamp> for String {
    fn from(x: UnsignedLustreTimestamp) -> String {
        x.to_string()
    }
}

/// Attempts to convert a string into an `UnsignedLustreTimestamp`.
///
/// The lustre timestamp can be in two formats:
/// 1. An `i64` representing the number of milliseconds since the Unix epoch.
/// 2. A string in the format "seconds.factional_seconds secs.[u|n]secs". For example,
///    "1409777887.590578 secs.usecs".
impl TryFrom<String> for UnsignedLustreTimestamp {
    type Error = LustreCollectorError;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        if let Ok(i) = s.parse::<i64>() {
            return Ok(Self(i));
        }

        let (time, format) = s
            .split_once(' ')
            .ok_or_else(|| LustreCollectorError::InvalidTime(s.to_string()))?;

        let (_unit, fractional_unit) = format
            .split_once('.')
            .ok_or_else(|| LustreCollectorError::InvalidTime(s.to_string()))?;

        let (secs, ss) = time
            .split_once('.')
            .ok_or_else(|| LustreCollectorError::InvalidTime(s.to_string()))?;

        let secs = secs
            .parse::<u64>()
            .map_err(|_| LustreCollectorError::InvalidTime(s.to_string()))?;

        let ss = ss
            .parse::<u32>()
            .map_err(|_| LustreCollectorError::InvalidTime(s.to_string()))?;

        let ns = match fractional_unit {
            "usecs" => ss * 1_000,
            "nsecs" => ss,
            _ => return Err(LustreCollectorError::InvalidTime(s.to_string())),
        };

        let d = Duration::new(secs, ns);

        let millis = u64::from(d.subsec_millis());

        let sec_millis = d.as_secs() * 1_000;

        let millis = i64::try_from(sec_millis + millis)
            .map_err(|_| LustreCollectorError::InvalidTime(s.to_string()))?;

        Ok(Self(millis))
    }
}

impl fmt::Display for UnsignedLustreTimestamp {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

pub mod lnet_exports {
    use std::collections::HashMap;

    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct LocalNiS {
        pub nid: String,
        pub status: String,
        pub statistics: LNetStatistics,
        pub sent_stats: Stats,
        pub received_stats: Stats,
        pub dropped_stats: Stats,
        #[serde(rename = "health stats")]
        pub health_stats: HealthStats,
        pub tunables: Tunables,
        #[serde(rename = "dev cpt")]
        pub dev_cpt: i64,
        #[serde(rename = "CPT")]
        pub cpt: String,
        pub interfaces: Option<HashMap<i64, String>>,
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct Stats {
        pub put: i64,
        pub get: i64,
        pub reply: i64,
        pub ack: i64,
        pub hello: i64,
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct HealthStats {
        #[serde(rename = "health value")]
        health_value: i64,
        interrupts: i64,
        dropped: i64,
        aborted: i64,
        #[serde(rename = "no route")]
        no_route: i64,
        timeouts: i64,
        error: i64,
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct HealthStatsPeer {
        #[serde(rename = "health value")]
        health_value: i64,
        dropped: i64,
        timeout: i64,
        error: i64,
        #[serde(rename = "network timeout")]
        network_timeout: i64,
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct Net {
        #[serde(rename = "net type")]
        pub net_type: String,
        #[serde(rename = "local NI(s)")]
        pub local_nis: Vec<LocalNiS>,
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct Global {
        numa_range: Option<i64>,
        max_intf: i64,
        discovery: i64,
        drop_asym_route: i64,
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct Peer {
        #[serde(rename = "primary nid")]
        pub primary_nid: String,
        #[serde(rename = "Multi-Rail")]
        pub multi_rail: String,
        #[serde(rename = "peer ni")]
        pub peer_ni: Vec<PeerNi>,
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct PeerNi {
        nid: String,
        state: String,
        max_ni_tx_credits: i64,
        available_tx_credits: i64,
        min_tx_credits: i64,
        tx_q_num_of_buf: i64,
        available_rtr_credits: i64,
        min_rtr_credits: i64,
        refcount: i64,
        statistics: LNetStatistics,
        sent_stats: Stats,
        received_stats: Stats,
        dropped_stats: Stats,
        #[serde(rename = "health stats")]
        health_stats: HealthStatsPeer,
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct LNetStatistics {
        pub send_count: i64,
        pub recv_count: i64,
        pub drop_count: i64,
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct LNetStatsStatistics {
        pub msgs_alloc: i64,
        pub msgs_max: i64,
        pub rst_alloc: i64,
        pub errors: i64,
        pub send_count: i64,
        pub resend_count: i64,
        pub response_timeout_count: i64,
        pub local_interrupt_count: i64,
        pub local_dropped_count: i64,
        pub local_aborted_count: i64,
        pub local_no_route_count: i64,
        pub local_timeout_count: i64,
        pub local_error_count: i64,
        pub remote_dropped_count: i64,
        pub remote_error_count: i64,
        pub remote_timeout_count: i64,
        pub network_timeout_count: i64,
        pub recv_count: i64,
        pub route_count: i64,
        pub drop_count: i64,
        pub send_length: i64,
        pub recv_length: i64,
        pub route_length: i64,
        pub drop_length: i64,
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct Tunables {
        pub peer_timeout: i64,
        pub peer_credits: i64,
        pub peer_buffer_credits: i64,
        pub credits: i64,
    }
}

#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct Stat {
    pub name: String,
    pub units: String,
    pub samples: u64,
    pub min: Option<u64>,
    pub max: Option<u64>,
    pub sum: Option<u64>,
    pub sumsquare: Option<u64>,
}

#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
/// A Stat specific to a host.
pub struct HostStat<T> {
    pub param: Param,
    pub value: T,
}

#[derive(PartialEq, Eq, PartialOrd, Ord, Debug, serde::Serialize, serde::Deserialize, Clone, Copy)]
pub enum TargetVariant {
    Ost,
    Mgt,
    Mdt,
}

impl fmt::Display for TargetVariant {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            TargetVariant::Ost => write!(f, "OST"),
            TargetVariant::Mgt => write!(f, "MGT"),
            TargetVariant::Mdt => write!(f, "MDT"),
        }
    }
}

impl Deref for TargetVariant {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        match *self {
            TargetVariant::Ost => "OST",
            TargetVariant::Mgt => "MGT",
            TargetVariant::Mdt => "MDT",
        }
    }
}

#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
/// Stats specific to a target.
pub struct TargetStat<T> {
    pub kind: TargetVariant,
    pub param: Param,
    pub target: Target,
    pub value: T,
}

#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
/// Stats from parsing `ost.OSS.<PARAM>.stats`
pub struct OssStat {
    pub param: Param,
    pub stats: Vec<Stat>,
}

#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
/// Stats from parsing `llite.*.stats`
pub struct LliteStat {
    pub target: Target,
    pub param: Param,
    pub stats: Vec<Stat>,
}

#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
/// Stats from parsing `mds.MDS.<PARAM>.stats`
pub struct MdsStat {
    pub param: Param,
    pub stats: Vec<Stat>,
}

#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
/// Stats specific to a quota target.
pub struct TargetQuotaStat<T> {
    pub pool: String,
    pub manager: String,
    pub param: Param,
    pub target: Target,
    pub value: T,
}

#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
/// Stats specific to a LNet Nid.
pub struct LNetStat<T> {
    pub nid: String,
    pub param: Param,
    pub value: T,
}

#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
/// Stats global across LNet Nids.
pub struct LNetStatGlobal<T> {
    pub param: Param,
    pub value: T,
}

#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
/// Changelog stats from parsing `mdd.*.changelog_users`.
pub struct ChangelogStat {
    pub current_index: u64,
    pub users: Vec<ChangeLogUser>,
}

#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct ChangeLogUser {
    pub user: String,
    pub index: u64,
    pub idle_secs: u64,
}

impl TryFrom<&Target> for TargetVariant {
    type Error = LustreCollectorError;

    fn try_from(x: &Target) -> Result<Self, Self::Error> {
        let x = x.deref().to_lowercase();
        let x = x.trim();

        if x == "mgs" {
            return Ok(TargetVariant::Mgt);
        }

        let target_name = x.rsplit_once('-').map(|(_, x)| x);

        match target_name {
            Some(x) if x.starts_with("ost") => Ok(TargetVariant::Ost),
            Some(x) if x.starts_with("mdt") => Ok(TargetVariant::Mdt),
            None | Some(_) => Err(LustreCollectorError::ConversionError(format!(
                "Could not convert {x} to target variant"
            ))),
        }
    }
}

#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct BrwStatsBucket {
    pub name: u64,
    pub read: u64,
    pub write: u64,
}

#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct BrwStats {
    pub name: String,
    pub unit: String,
    pub buckets: Vec<BrwStatsBucket>,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub enum RecoveryStatus {
    Complete,
    Inactive,
    Waiting,
    WaitingForClients,
    Recovering,
    Unknown,
}

#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum HostStats {
    MemusedMax(HostStat<u64>),
    Memused(HostStat<u64>),
    LNetMemUsed(HostStat<u64>),
    HealthCheck(HostStat<HealthCheckStat>),
}

#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct HealthCheckStat {
    pub healthy: bool,
    pub targets: Vec<Target>,
}

/// A Stat specific to a node.
#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct NodeStat<T> {
    pub param: Param,
    pub value: T,
}
/// Top level node stats (not directly Lustre related)
#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum NodeStats {
    CpuUser(NodeStat<u64>),
    CpuSystem(NodeStat<u64>),
    CpuIowait(NodeStat<u64>),
    CpuTotal(NodeStat<u64>),
    MemTotal(NodeStat<u64>),
    MemFree(NodeStat<u64>),
    SwapTotal(NodeStat<u64>),
    SwapFree(NodeStat<u64>),
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FsName(pub String);

/// The target stats currently collected
#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum TargetStats {
    /// Operations per OST. Read and write data is particularly interesting
    Stats(TargetStat<Vec<Stat>>),
    BrwStats(TargetStat<Vec<BrwStats>>),
    /// Available inodes
    FilesFree(TargetStat<u64>),
    /// Total inodes
    FilesTotal(TargetStat<u64>),
    /// Type of target
    FsType(TargetStat<String>),
    /// Available disk space
    KBytesAvail(TargetStat<u64>),
    /// Free disk space
    KBytesFree(TargetStat<u64>),
    /// Total disk space
    KBytesTotal(TargetStat<u64>),
    NumExports(TargetStat<u64>),
    TotDirty(TargetStat<u64>),
    TotGranted(TargetStat<u64>),
    TotPending(TargetStat<u64>),
    ContendedLocks(TargetStat<u64>),
    ContentionSeconds(TargetStat<u64>),
    ConnectedClients(TargetStat<u64>),
    CtimeAgeLimit(TargetStat<u64>),
    EarlyLockCancel(TargetStat<u64>),
    FsNames(TargetStat<Vec<FsName>>),
    LockCount(TargetStat<u64>),
    LockTimeouts(TargetStat<u64>),
    LockUnusedCount(TargetStat<u64>),
    LruMaxAge(TargetStat<u64>),
    LruSize(TargetStat<u64>),
    MaxNolockBytes(TargetStat<u64>),
    MaxParallelAst(TargetStat<u64>),
    ResourceCount(TargetStat<u64>),
    ThreadsMin(TargetStat<u64>),
    ThreadsMax(TargetStat<u64>),
    ThreadsStarted(TargetStat<u64>),
    Oss(OssStat),
    RecoveryCompletedClients(TargetStat<u64>),
    RecoveryConnectedClients(TargetStat<u64>),
    RecoveryDuration(TargetStat<u64>),
    RecoveryEvictedClients(TargetStat<u64>),
    RecoveryStatus(TargetStat<RecoveryStatus>),
    RecoveryTimeRemaining(TargetStat<u64>),
    RecoveryTotalClients(TargetStat<u64>),
    Llite(LliteStat),
    ExportStats(TargetStat<Vec<ExportStats>>),
    Mds(MdsStat),
    Changelog(TargetStat<ChangelogStat>),
    QuotaStats(TargetQuotaStat<QuotaStats>),
    QuotaStatsOsd(TargetStat<QuotaStatsOsd>),
}

#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum LNetStats {
    SendCount(LNetStat<i64>),
    RecvCount(LNetStat<i64>),
    DropCount(LNetStat<i64>),
    SendLength(LNetStatGlobal<i64>),
    RecvLength(LNetStatGlobal<i64>),
    DropLength(LNetStatGlobal<i64>),
}

#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum LustreServiceStats {
    LdlmCanceld(Vec<Stat>),
    LdlmCbd(Vec<Stat>),
}

#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Record {
    Host(HostStats),
    LNetStat(LNetStats),
    LustreService(LustreServiceStats),
    Node(NodeStats),
    Target(TargetStats),
}

#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct QuotaStatLimits {
    pub hard: u64,
    pub soft: u64,
    pub granted: u64,
    pub time: u64,
}

#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct QuotaStatUsage {
    pub inodes: u64,
    pub kbytes: u64,
}

#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct QuotaStat {
    pub id: u64,
    pub limits: QuotaStatLimits,
}

#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct QuotaStatOsd {
    pub id: u64,
    pub usage: QuotaStatUsage,
}

#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct QuotaStats {
    pub kind: QuotaKind,
    pub stats: Vec<QuotaStat>,
}

#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub struct QuotaStatsOsd {
    pub kind: QuotaKind,
    pub stats: Vec<QuotaStatOsd>,
}

#[derive(PartialEq, Eq, Debug, serde::Serialize, serde::Deserialize)]
pub enum QuotaKind {
    Usr,
    Grp,
    Prj,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::convert::TryInto;

    #[test]
    fn test_unsigned_lustre_timestamp_try_from() {
        let s = "0.590578 secs.usecs".to_string();
        let timestamp: Result<UnsignedLustreTimestamp, _> = s.try_into();

        match timestamp {
            Ok(t) => assert_eq!((t.0), 590),
            Err(e) => panic!("Error occurred: {:?}", e),
        }

        let s = "1709305846.694991088 secs.nsecs".to_string();
        let timestamp: Result<UnsignedLustreTimestamp, _> = s.try_into();

        match timestamp {
            Ok(t) => assert_eq!((t.0), 1709305846694),
            Err(e) => panic!("Error occurred: {:?}", e),
        }

        let s = "1709305846694".to_string();
        let timestamp: Result<UnsignedLustreTimestamp, _> = s.try_into();

        match timestamp {
            Ok(t) => assert_eq!((t.0), 1709305846694),
            Err(e) => panic!("Error occurred: {:?}", e),
        }
    }
}

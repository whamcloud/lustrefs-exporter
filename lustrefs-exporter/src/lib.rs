// Copyright (c) 2024 DDN. All rights reserved.
// Use of this source code is governed by a MIT-style
// license that can be found in the LICENSE file.

pub mod brw_stats;
pub mod host;
pub mod jobstats;
pub mod llite;
pub mod lnet;
pub mod quota;
pub mod service;
pub mod stats;

use brw_stats::build_target_stats;
use host::build_host_stats;
use lnet::build_lnet_stats;
use lustre_collector::{
    HostStat, JobStatMdt, JobStatOst, LNetStat, LNetStatGlobal, Record, TargetStat, TargetVariant,
};
use num_traits::Num;
use prometheus_client::{
    encoding::{text::encode, EncodeLabelSet, LabelSetEncoder},
    metrics::{family::Family, gauge::Gauge},
    registry::Registry,
};
use prometheus_exporter_base::{prelude::*, Yes};
use service::build_service_stats;
use std::{collections::BTreeMap, fmt, ops::Deref};

#[derive(Debug, Clone, Copy)]
struct Metric {
    name: &'static str,
    help: &'static str,
    r#type: MetricType,
}

trait LabelProm {
    fn to_prom_label(&self) -> &'static str;
}

impl LabelProm for TargetVariant {
    fn to_prom_label(&self) -> &'static str {
        match self {
            TargetVariant::Ost => "ost",
            TargetVariant::Mgt => "mgt",
            TargetVariant::Mdt => "mdt",
        }
    }
}

impl From<Metric> for PrometheusMetric<'_> {
    fn from(x: Metric) -> Self {
        PrometheusMetric::build()
            .with_name(x.name)
            .with_help(x.help)
            .with_metric_type(x.r#type)
            .build()
    }
}

trait ToMetricInst<T>
where
    T: Num + fmt::Display + fmt::Debug + Copy,
{
    fn to_metric_inst(&self) -> PrometheusInstance<'_, T, Yes>;
}

impl<T> ToMetricInst<T> for TargetStat<T>
where
    T: Num + fmt::Display + fmt::Debug + Copy,
{
    fn to_metric_inst(&self) -> PrometheusInstance<'_, T, Yes> {
        PrometheusInstance::new()
            .with_label("component", self.kind.to_prom_label())
            .with_label("target", self.target.deref())
            .with_value(self.value)
    }
}

impl<T> ToMetricInst<T> for LNetStat<T>
where
    T: Num + fmt::Display + fmt::Debug + Copy,
{
    fn to_metric_inst(&self) -> PrometheusInstance<'_, T, Yes> {
        PrometheusInstance::new()
            .with_label("nid", self.nid.deref())
            .with_value(self.value)
    }
}

impl<T> ToMetricInst<T> for LNetStatGlobal<T>
where
    T: Num + fmt::Display + fmt::Debug + Copy,
{
    fn to_metric_inst(&self) -> PrometheusInstance<'_, T, Yes> {
        PrometheusInstance::new().with_value(self.value)
    }
}

impl<T> ToMetricInst<T> for HostStat<T>
where
    T: Num + fmt::Display + fmt::Debug + Copy,
{
    fn to_metric_inst(&self) -> PrometheusInstance<'_, T, Yes> {
        PrometheusInstance::new().with_value(self.value)
    }
}

trait Name {
    fn name(&self) -> &'static str;
}

impl Name for Metric {
    fn name(&self) -> &'static str {
        self.name
    }
}

trait StatsMapExt {
    fn get_mut_metric<T: Name + Into<PrometheusMetric<'static>>>(
        &mut self,
        x: T,
    ) -> &mut PrometheusMetric<'static>;
}

impl StatsMapExt for BTreeMap<&'static str, PrometheusMetric<'static>> {
    fn get_mut_metric<T: Name + Into<PrometheusMetric<'static>>>(
        &mut self,
        x: T,
    ) -> &mut PrometheusMetric<'static> {
        self.entry(x.name()).or_insert_with(|| x.into())
    }
}

pub fn build_lustre_stats(output: Vec<Record>) -> String {
    let mut stats_map = BTreeMap::new();

    for x in output {
        match x {
            lustre_collector::Record::Host(x) => {
                build_host_stats(x, &mut stats_map);
            }
            lustre_collector::Record::Node(_) => {}
            lustre_collector::Record::LNetStat(x) => {
                build_lnet_stats(x, &mut stats_map);
            }
            lustre_collector::Record::Target(x) => {
                build_target_stats(x, &mut stats_map);
            }
            lustre_collector::Record::LustreService(x) => {
                build_service_stats(x, &mut stats_map);
            }
        }
    }

    stats_map
        .values()
        .map(|x| x.render())
        .collect::<Vec<_>>()
        .join("\n")
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct LabelsOst<'a> {
    component: &'a str,
    target: String,
    jobid: &'a str,
}

impl<'a> EncodeLabelSet for LabelsOst<'a> {
    fn encode(&self, mut encoder: LabelSetEncoder) -> std::result::Result<(), std::fmt::Error> {
        use prometheus_client::encoding::EncodeLabelKey;
        use prometheus_client::encoding::EncodeLabelValue;

        let mut encode = |key: &str, value: &str| -> std::result::Result<(), std::fmt::Error> {
            let mut label_encoder = encoder.encode_label();
            let mut label_key_encoder = label_encoder.encode_label_key()?;
            EncodeLabelKey::encode(&key, &mut label_key_encoder)?;
            let mut label_value_encoder = label_key_encoder.encode_label_value()?;
            EncodeLabelValue::encode(&value, &mut label_value_encoder)?;
            label_value_encoder.finish()
        };

        encode("component", self.component)?;
        encode("target", &self.target)?;
        encode("jobid", self.jobid)?;

        Ok(())
    }
}

#[derive(Clone, Debug, Hash, PartialEq, Eq)]
struct LabelsMdt<'a> {
    component: &'a str,
    target: String,
    jobid: &'a str,
    operation: &'a str,
}

impl<'a> EncodeLabelSet for LabelsMdt<'a> {
    fn encode(&self, mut encoder: LabelSetEncoder) -> std::result::Result<(), std::fmt::Error> {
        use prometheus_client::encoding::EncodeLabelKey;
        use prometheus_client::encoding::EncodeLabelValue;

        let mut encode = |key: &str, value: &str| -> std::result::Result<(), std::fmt::Error> {
            let mut label_encoder = encoder.encode_label();
            let mut label_key_encoder = label_encoder.encode_label_key()?;
            EncodeLabelKey::encode(&key, &mut label_key_encoder)?;
            let mut label_value_encoder = label_key_encoder.encode_label_value()?;
            EncodeLabelValue::encode(&value, &mut label_value_encoder)?;
            label_value_encoder.finish()
        };

        encode("component", self.component)?;
        encode("target", &self.target)?;
        encode("jobid", self.jobid)?;
        encode("operation", self.operation)?;

        Ok(())
    }
}

pub fn build_lustre_stats_new(output: &Vec<Record<'_>>) -> String {
    let mut registry = Registry::default();

    // Register the metric family with the registry.
    let lustre_job_read_samples_total = Family::<LabelsOst, Gauge>::default();
    registry.register(
        // With the metric name.
        "lustre_job_read_samples_total",
        // And the metric help text.
        "Total number of reads that have been recorded",
        lustre_job_read_samples_total.clone(),
    );

    let lustre_job_read_minimum_size_bytes = Family::<LabelsOst, Gauge>::default();
    registry.register(
        // With the metric name.
        "lustre_job_read_minimum_size_bytes",
        // And the metric help text.
        "The minimum read size in bytes",
        lustre_job_read_minimum_size_bytes.clone(),
    );

    let lustre_job_read_maximum_size_bytes = Family::<LabelsOst, Gauge>::default();
    registry.register(
        // With the metric name.
        "lustre_job_read_maximum_size_bytes",
        // And the metric help text.
        "The maximum read size in bytes",
        lustre_job_read_maximum_size_bytes.clone(),
    );

    let lustre_job_read_bytes_total = Family::<LabelsOst, Gauge>::default();
    registry.register(
        // With the metric name.
        "lustre_job_read_bytes_total",
        // And the metric help text.
        "The total number of bytes that have been read",
        lustre_job_read_bytes_total.clone(),
    );

    let lustre_job_write_samples_total = Family::<LabelsOst, Gauge>::default();
    registry.register(
        // With the metric name.
        "lustre_job_write_samples_total",
        // And the metric help text.
        "Total number of writes that have been recorded",
        lustre_job_write_samples_total.clone(),
    );

    let lustre_job_write_minimum_size_bytes = Family::<LabelsOst, Gauge>::default();
    registry.register(
        // With the metric name.
        "lustre_job_write_minimum_size_bytes",
        // And the metric help text.
        "The minimum write size in bytes",
        lustre_job_write_minimum_size_bytes.clone(),
    );

    let lustre_job_write_maximum_size_bytes = Family::<LabelsOst, Gauge>::default();
    registry.register(
        // With the metric name.
        "lustre_job_write_maximum_size_bytes",
        // And the metric help text.
        "The maximum write size in bytes",
        lustre_job_write_maximum_size_bytes.clone(),
    );

    let lustre_job_write_bytes_total = Family::<LabelsOst, Gauge>::default();
    registry.register(
        // With the metric name.
        "lustre_job_write_bytes_total",
        // And the metric help text.
        "The total number of bytes that have been written",
        lustre_job_write_bytes_total.clone(),
    );

    let lustre_job_stats_total = Family::<LabelsMdt, Gauge>::default();
    registry.register(
        // With the metric name.
        "lustre_job_stats_total",
        // And the metric help text.
        "Number of operations the filesystem has performed, recorded by jobstats",
        lustre_job_stats_total.clone(),
    );

    for x in output {
        match x {
            lustre_collector::Record::Target(x) => match x {
                lustre_collector::TargetStats::JobStatsOst(x) => build_ost_job_stats_new(
                    x,
                    &lustre_job_read_samples_total,
                    &lustre_job_read_minimum_size_bytes,
                    &lustre_job_read_maximum_size_bytes,
                    &lustre_job_read_bytes_total,
                    &lustre_job_write_samples_total,
                    &lustre_job_write_minimum_size_bytes,
                    &lustre_job_write_maximum_size_bytes,
                    &lustre_job_write_bytes_total,
                    &lustre_job_stats_total,
                ),
                lustre_collector::TargetStats::JobStatsMdt(x) => {
                    build_mdt_job_stats_new(x, &lustre_job_stats_total)
                }
                _ => continue,
            },
            _ => continue,
        }
    }

    let mut buffer = String::new();
    encode(&mut buffer, &registry).unwrap();

    buffer
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn build_ost_job_stats_new<'a>(
    x: &TargetStat<Option<Vec<JobStatOst<'a>>>>,
    lustre_job_read_samples_total: &Family<LabelsOst<'a>, Gauge>,
    lustre_job_read_minimum_size_bytes: &Family<LabelsOst<'a>, Gauge>,
    lustre_job_read_maximum_size_bytes: &Family<LabelsOst<'a>, Gauge>,
    lustre_job_read_bytes_total: &Family<LabelsOst<'a>, Gauge>,
    lustre_job_write_samples_total: &Family<LabelsOst<'a>, Gauge>,
    lustre_job_write_minimum_size_bytes: &Family<LabelsOst<'a>, Gauge>,
    lustre_job_write_maximum_size_bytes: &Family<LabelsOst<'a>, Gauge>,
    lustre_job_write_bytes_total: &Family<LabelsOst<'a>, Gauge>,
    lustre_job_stats_total: &Family<LabelsMdt<'a>, Gauge>,
) {
    let TargetStat {
        kind,
        target,
        value,
        ..
    } = x;

    let xs = match value {
        Some(xs) => xs,
        None => return,
    };

    for x in xs {
        let JobStatOst {
            job_id,
            snapshot_time: _,
            start_time: _,
            elapsed_time: _,
            read_bytes,
            write_bytes,
            getattr,
            setattr,
            punch,
            sync,
            destroy,
            create,
            statfs,
            get_info,
            set_info,
            quotactl,
        } = x;

        lustre_job_read_samples_total
            .get_or_create(&LabelsOst {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
            })
            .set(read_bytes.samples.parse().unwrap());

        lustre_job_read_minimum_size_bytes
            .get_or_create(&LabelsOst {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
            })
            .set(read_bytes.min.parse().unwrap());

        lustre_job_read_maximum_size_bytes
            .get_or_create(&LabelsOst {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
            })
            .set(read_bytes.max.parse().unwrap());

        lustre_job_read_bytes_total
            .get_or_create(&LabelsOst {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
            })
            .set(read_bytes.sum.parse().unwrap());

        lustre_job_write_samples_total
            .get_or_create(&LabelsOst {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
            })
            .set(write_bytes.samples.parse().unwrap());
        lustre_job_write_minimum_size_bytes
            .get_or_create(&LabelsOst {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
            })
            .set(write_bytes.min.parse().unwrap());
        lustre_job_write_maximum_size_bytes
            .get_or_create(&LabelsOst {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
            })
            .set(write_bytes.max.parse().unwrap());
        lustre_job_write_bytes_total
            .get_or_create(&LabelsOst {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
            })
            .set(write_bytes.sum.parse().unwrap());

        lustre_job_stats_total
            .get_or_create(&LabelsMdt {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
                operation: "getattr",
            })
            .set(getattr.samples.parse().unwrap());

        lustre_job_stats_total
            .get_or_create(&LabelsMdt {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
                operation: "setattr",
            })
            .set(setattr.samples.parse().unwrap());

        lustre_job_stats_total
            .get_or_create(&LabelsMdt {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
                operation: "punch",
            })
            .set(punch.samples.parse().unwrap());

        lustre_job_stats_total
            .get_or_create(&LabelsMdt {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
                operation: "sync",
            })
            .set(sync.samples.parse().unwrap());

        lustre_job_stats_total
            .get_or_create(&LabelsMdt {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
                operation: "destroy",
            })
            .set(destroy.samples.parse().unwrap());

        lustre_job_stats_total
            .get_or_create(&LabelsMdt {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
                operation: "create",
            })
            .set(create.samples.parse().unwrap());

        lustre_job_stats_total
            .get_or_create(&LabelsMdt {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
                operation: "statfs",
            })
            .set(statfs.samples.parse().unwrap());

        lustre_job_stats_total
            .get_or_create(&LabelsMdt {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
                operation: "get_info",
            })
            .set(get_info.samples.parse().unwrap());

        lustre_job_stats_total
            .get_or_create(&LabelsMdt {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
                operation: "set_info",
            })
            .set(set_info.samples.parse().unwrap());

        lustre_job_stats_total
            .get_or_create(&LabelsMdt {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
                operation: "quotactl",
            })
            .set(quotactl.samples.parse().unwrap());
    }
}

pub(crate) fn build_mdt_job_stats_new<'a>(
    x: &TargetStat<Option<Vec<JobStatMdt<'a>>>>,
    lustre_job_stats_total: &Family<LabelsMdt<'a>, Gauge>,
) {
    let TargetStat {
        kind,
        target,
        value,
        ..
    } = x;

    let xs = match value {
        Some(xs) => xs,
        None => return,
    };

    for x in xs {
        let JobStatMdt {
            job_id,
            snapshot_time: _,
            start_time: _,
            elapsed_time: _,
            open,
            close,
            mknod,
            link,
            unlink,
            mkdir,
            rmdir,
            rename,
            getattr,
            setattr,
            getxattr,
            setxattr,
            statfs,
            sync,
            samedir_rename,
            crossdir_rename,
            read_bytes,
            write_bytes,
            punch,
            parallel_rename_dir: _,
            parallel_rename_file: _,
        } = x;

        lustre_job_stats_total
            .get_or_create(&LabelsMdt {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
                operation: "open",
            })
            .set(open.samples.parse().unwrap());

        lustre_job_stats_total
            .get_or_create(&LabelsMdt {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
                operation: "close",
            })
            .set(close.samples.parse().unwrap());

        lustre_job_stats_total
            .get_or_create(&LabelsMdt {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
                operation: "mknod",
            })
            .set(mknod.samples.parse().unwrap());

        lustre_job_stats_total
            .get_or_create(&LabelsMdt {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
                operation: "link",
            })
            .set(link.samples.parse().unwrap());

        lustre_job_stats_total
            .get_or_create(&LabelsMdt {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
                operation: "unlink",
            })
            .set(unlink.samples.parse().unwrap());

        lustre_job_stats_total
            .get_or_create(&LabelsMdt {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
                operation: "mkdir",
            })
            .set(mkdir.samples.parse().unwrap());

        lustre_job_stats_total
            .get_or_create(&LabelsMdt {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
                operation: "rmdir",
            })
            .set(rmdir.samples.parse().unwrap());

        lustre_job_stats_total
            .get_or_create(&LabelsMdt {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
                operation: "rename",
            })
            .set(rename.samples.parse().unwrap());

        lustre_job_stats_total
            .get_or_create(&LabelsMdt {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
                operation: "getattr",
            })
            .set(getattr.samples.parse().unwrap());

        lustre_job_stats_total
            .get_or_create(&LabelsMdt {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
                operation: "setattr",
            })
            .set(setattr.samples.parse().unwrap());

        lustre_job_stats_total
            .get_or_create(&LabelsMdt {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
                operation: "getxattr",
            })
            .set(getxattr.samples.parse().unwrap());

        lustre_job_stats_total
            .get_or_create(&LabelsMdt {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
                operation: "setxattr",
            })
            .set(setxattr.samples.parse().unwrap());

        lustre_job_stats_total
            .get_or_create(&LabelsMdt {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
                operation: "rmdir",
            })
            .set(rmdir.samples.parse().unwrap());

        lustre_job_stats_total
            .get_or_create(&LabelsMdt {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
                operation: "statfs",
            })
            .set(statfs.samples.parse().unwrap());

        lustre_job_stats_total
            .get_or_create(&LabelsMdt {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
                operation: "sync",
            })
            .set(sync.samples.parse().unwrap());

        lustre_job_stats_total
            .get_or_create(&LabelsMdt {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
                operation: "samedir_rename",
            })
            .set(samedir_rename.samples.parse().unwrap());

        lustre_job_stats_total
            .get_or_create(&LabelsMdt {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
                operation: "crossdir_rename",
            })
            .set(crossdir_rename.samples.parse().unwrap());

        lustre_job_stats_total
            .get_or_create(&LabelsMdt {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
                operation: "read_bytes",
            })
            .set(read_bytes.samples.parse().unwrap());

        lustre_job_stats_total
            .get_or_create(&LabelsMdt {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
                operation: "write_bytes",
            })
            .set(write_bytes.samples.parse().unwrap());
        lustre_job_stats_total
            .get_or_create(&LabelsMdt {
                component: kind.to_prom_label(),
                target: target.deref().to_string(),
                jobid: job_id,
                operation: "punch",
            })
            .set(punch.samples.parse().unwrap());
    }
}

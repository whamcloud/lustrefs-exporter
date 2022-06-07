pub mod brw_stats;
pub mod jobstats;
pub mod lnet;

use brw_stats::build_target_stats;
use lnet::build_lnet_stats;
use lustre_collector::{LNetStat, Record, TargetStat};
use num_traits::Num;
use prometheus_exporter_base::{prelude::*, Yes};
use std::{collections::BTreeMap, fmt, ops::Deref, time::Duration};

#[derive(Debug, Clone, Copy)]
struct Metric {
    name: &'static str,
    help: &'static str,
    r#type: MetricType,
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
    fn to_metric_inst(&self, time: Duration) -> PrometheusInstance<'_, T, Yes>;
}

impl<T> ToMetricInst<T> for TargetStat<T>
where
    T: Num + fmt::Display + fmt::Debug + Copy,
{
    fn to_metric_inst(&self, time: Duration) -> PrometheusInstance<'_, T, Yes> {
        PrometheusInstance::new()
            .with_label("component", self.kind.deref())
            .with_label("target", self.target.deref())
            .with_value(self.value)
            .with_timestamp(time.as_millis())
    }
}

impl<T> ToMetricInst<T> for LNetStat<T>
where
    T: Num + fmt::Display + fmt::Debug + Copy,
{
    fn to_metric_inst(&self, time: Duration) -> PrometheusInstance<'_, T, Yes> {
        PrometheusInstance::new()
            .with_label("nid", self.nid.deref())
            .with_value(self.value)
            .with_timestamp(time.as_millis())
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

pub fn build_lustre_stats(output: Vec<Record>, time: Duration) -> String {
    let mut stats_map = BTreeMap::new();

    for x in output {
        match x {
            lustre_collector::Record::Host(_) => {}
            lustre_collector::Record::Node(_) => {}
            lustre_collector::Record::LNetStat(x) => {
                build_lnet_stats(x, &mut stats_map, time);
            }
            lustre_collector::Record::Target(x) => {
                build_target_stats(x, &mut stats_map, time);
            }
        }
    }

    stats_map
        .values()
        .map(|x| x.render())
        .collect::<Vec<_>>()
        .join("\n")
}

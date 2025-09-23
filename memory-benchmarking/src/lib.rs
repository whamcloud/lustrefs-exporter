use std::time::Duration;

use sysinfo::{Pid, ProcessExt, System, SystemExt};

#[derive(serde::Serialize)]
pub struct BencherOutput {
    memory_usage: BencherMetrics,
}

#[derive(serde::Serialize)]
struct BencherMetrics {
    start_rss_mib: MetricEntry,
    peak_rss_mib: MetricEntry,
    end_rss_mib: MetricEntry,
    memory_growth_mib: MetricEntry,
    peak_over_start_rss_ratio: MetricEntry,
    avg_runtime_rss_mib: MetricEntry,
    start_virtual_mib: MetricEntry,
    peak_virtual_mib: MetricEntry,
    end_virtual_mib: MetricEntry,
    virtual_growth_mib: MetricEntry,
    peak_over_start_virtual_ratio: MetricEntry,
    avg_runtime_virtual_mib: MetricEntry,
}

#[derive(serde::Serialize, Default)]
struct MetricEntry {
    value: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    lower_value: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    upper_value: Option<f64>,
}

static MIB: f64 = 1024f64 * 1024f64;

impl MetricEntry {
    fn bytes_as_mib(self) -> Self {
        MetricEntry {
            value: self.value / MIB,
            lower_value: self.lower_value.map(|v| v / MIB),
            upper_value: self.upper_value.map(|v| v / MIB),
        }
    }
}

#[derive(Clone, Debug, serde::Serialize)]
pub struct MemoryUsage {
    pub start_rss: f64,
    pub end_rss: f64,
    pub avg_rss: f64,
    pub min_rss: f64,
    pub max_rss: f64,
    pub start_virtual: f64,
    pub end_virtual: f64,
    pub avg_virtual: f64,
    pub min_virtual: f64,
    pub max_virtual: f64,
}

impl MemoryUsage {
    fn memory_growth(&self) -> f64 {
        self.end_rss - self.start_rss
    }

    fn peak_over_start_rss(&self) -> f64 {
        self.max_rss / self.start_rss
    }

    fn virtual_growth(&self) -> f64 {
        self.end_virtual - self.start_virtual
    }

    fn peak_over_start_virtual(&self) -> f64 {
        self.max_virtual / self.start_virtual
    }
}

trait GetStatsExt {
    fn get_stats(&self, accessor: impl Fn(&MemoryUsage) -> f64 + Copy) -> MetricEntry;
}

impl GetStatsExt for &[MemoryUsage] {
    fn get_stats(&self, accessor: impl Fn(&MemoryUsage) -> f64 + Copy) -> MetricEntry {
        MetricEntry {
            value: self.iter().map(accessor).sum::<f64>() / self.len() as f64,
            lower_value: self.iter().map(accessor).reduce(f64::min),
            upper_value: self.iter().map(accessor).reduce(f64::max),
        }
    }
}

impl From<&[MemoryUsage]> for BencherOutput {
    fn from(samples: &[MemoryUsage]) -> Self {
        BencherOutput {
            memory_usage: BencherMetrics {
                start_rss_mib: samples.get_stats(|x| x.start_rss).bytes_as_mib(),
                peak_rss_mib: samples.get_stats(|x| x.max_rss).bytes_as_mib(),
                end_rss_mib: samples.get_stats(|x| x.end_rss).bytes_as_mib(),
                memory_growth_mib: samples.get_stats(|x| x.memory_growth()).bytes_as_mib(),
                peak_over_start_rss_ratio: samples
                    .get_stats(|x| x.peak_over_start_rss())
                    .bytes_as_mib(),
                avg_runtime_rss_mib: samples.get_stats(|x| x.avg_rss).bytes_as_mib(),
                start_virtual_mib: samples.get_stats(|x| x.start_virtual).bytes_as_mib(),
                peak_virtual_mib: samples.get_stats(|x| x.max_virtual).bytes_as_mib(),
                end_virtual_mib: samples.get_stats(|x| x.end_virtual).bytes_as_mib(),
                virtual_growth_mib: samples.get_stats(|x| x.virtual_growth()).bytes_as_mib(),
                peak_over_start_virtual_ratio: samples
                    .get_stats(|x| x.peak_over_start_virtual())
                    .bytes_as_mib(),
                avg_runtime_virtual_mib: samples.get_stats(|x| x.avg_virtual).bytes_as_mib(),
            },
        }
    }
}

#[derive(Default)]
pub struct Sample(f64, f64);

#[derive(thiserror::Error, Debug, Clone)]
pub enum Error {
    #[error("Not enough samples")]
    NotEnoughSamples,
}

impl TryFrom<&[Sample]> for MemoryUsage {
    type Error = Error;

    fn try_from(value: &[Sample]) -> Result<Self, Self::Error> {
        if value.len() < 10 {
            return Err(Error::NotEnoughSamples);
        }

        Ok(MemoryUsage {
            start_rss: value.first().map(|s| s.0).unwrap_or_default(),
            end_rss: value.last().map(|s| s.0).unwrap_or_default(),
            avg_rss: value.iter().map(|s| s.0).sum::<f64>() / (value.len() as f64),
            min_rss: value
                .iter()
                .map(|s| s.0)
                .reduce(f64::min)
                .unwrap_or_default(),
            max_rss: value
                .iter()
                .map(|s| s.0)
                .reduce(f64::max)
                .unwrap_or_default(),
            start_virtual: value.first().map(|s| s.1).unwrap_or_default(),
            end_virtual: value.last().map(|s| s.1).unwrap_or_default(),
            avg_virtual: value.iter().map(|s| s.1).sum::<f64>() / (value.len() as f64),
            min_virtual: value
                .iter()
                .map(|s| s.1)
                .reduce(f64::min)
                .unwrap_or_default(),
            max_virtual: value
                .iter()
                .map(|s| s.1)
                .reduce(f64::max)
                .unwrap_or_default(),
        })
    }
}

pub fn sample_memory() -> Sample {
    let mut system = System::new();
    system.refresh_process(Pid::from(std::process::id() as usize));

    system
        .process(Pid::from(std::process::id() as usize))
        .map(|process| Sample(process.memory() as f64, process.virtual_memory() as f64))
        .unwrap_or_default()
}

pub fn trace_memory(routine: impl Fn(), duration: Duration) -> Vec<Sample> {
    tokio::runtime::Builder::new_multi_thread()
        .enable_time()
        .enable_io()
        .build()
        .expect("Failed to build tokio runtime")
        .block_on(async move {
            let mut samples = vec![sample_memory()];

            let (abort_sender, mut abort_receiver) = tokio::sync::oneshot::channel::<()>();
            let monitor_handle = tokio::spawn(async move {
                let mut interval = tokio::time::interval(duration);

                while abort_receiver.try_recv().is_err() {
                    interval.tick().await;

                    samples.push(sample_memory());
                }

                samples
            });

            routine();

            abort_sender
                .send(())
                .expect("Failed to send stop signal to memory monitor");

            let mut samples = monitor_handle
                .await
                .expect("Failed to collect memory metrics from run.");

            samples.push(sample_memory());

            samples
        })
}

pub fn trace_memory_async<I, F>(
    init: impl Fn() -> I + Send + 'static,
    routine: impl Fn() -> F,
    duration: Duration,
) -> Vec<Sample>
where
    I: Future<Output = ()> + Sized + Send + 'static,
    F: Future<Output = ()> + Sized,
{
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_time()
        .enable_io()
        .build()
        .expect("Failed to build tokio runtime");

    rt.spawn(init());

    let mut samples = vec![sample_memory()];

    rt.block_on(async move {
        let (abort_sender, mut abort_receiver) = tokio::sync::oneshot::channel::<()>();
        let monitor_handle = tokio::spawn(async move {
            let mut interval = tokio::time::interval(duration);

            while abort_receiver.try_recv().is_err() {
                interval.tick().await;

                samples.push(sample_memory());
            }

            samples
        });

        routine().await;

        abort_sender
            .send(())
            .expect("Failed to send stop signal to memory monitor");

        let mut samples = monitor_handle
            .await
            .expect("Failed to collect memory metrics from run.");

        samples.push(sample_memory());

        samples
    })
}

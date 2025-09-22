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

impl From<MemoryUsage> for BencherOutput {
    fn from(x: MemoryUsage) -> Self {
        BencherOutput {
            memory_usage: BencherMetrics {
                start_rss_mib: MetricEntry {
                    value: x.start_rss,
                    ..Default::default()
                },
                peak_rss_mib: MetricEntry {
                    value: x.max_rss,
                    ..Default::default()
                },
                end_rss_mib: MetricEntry {
                    value: x.end_rss,
                    ..Default::default()
                },
                memory_growth_mib: MetricEntry {
                    value: x.memory_growth(),
                    ..Default::default()
                },
                peak_over_start_rss_ratio: MetricEntry {
                    value: x.peak_over_start_rss(),
                    ..Default::default()
                },
                avg_runtime_rss_mib: MetricEntry {
                    value: x.avg_rss,
                    lower_value: Some(x.min_rss),
                    upper_value: Some(x.max_rss),
                },
                start_virtual_mib: MetricEntry {
                    value: x.start_virtual,
                    ..Default::default()
                },
                peak_virtual_mib: MetricEntry {
                    value: x.max_virtual,
                    ..Default::default()
                },
                end_virtual_mib: MetricEntry {
                    value: x.end_virtual,
                    ..Default::default()
                },
                virtual_growth_mib: MetricEntry {
                    value: x.virtual_growth(),
                    ..Default::default()
                },
                peak_over_start_virtual_ratio: MetricEntry {
                    value: x.peak_over_start_virtual(),
                    ..Default::default()
                },
                avg_runtime_virtual_mib: MetricEntry {
                    value: x.avg_virtual,
                    lower_value: Some(x.min_virtual),
                    upper_value: Some(x.max_virtual),
                },
            },
        }
    }
}

pub fn aggregate_samples(samples: &[MemoryUsage]) -> MemoryUsage {
    let size = samples.len() as f64;
    const MIB_CONVERSION_FACTOR: f64 = 1_048_576.0;

    MemoryUsage {
        start_rss: samples.iter().map(|x| x.start_rss).sum::<f64>() / size / MIB_CONVERSION_FACTOR,
        end_rss: samples.iter().map(|x| x.end_rss).sum::<f64>() / size / MIB_CONVERSION_FACTOR,
        avg_rss: samples.iter().map(|x| x.avg_rss).sum::<f64>() / size / MIB_CONVERSION_FACTOR,
        min_rss: samples
            .iter()
            .map(|x| x.min_rss)
            .fold(f64::INFINITY, f64::min)
            / MIB_CONVERSION_FACTOR,
        max_rss: samples
            .iter()
            .map(|x| x.max_rss)
            .fold(f64::NEG_INFINITY, f64::max)
            / MIB_CONVERSION_FACTOR,

        start_virtual: samples.iter().map(|x| x.start_virtual).sum::<f64>()
            / size
            / MIB_CONVERSION_FACTOR,
        end_virtual: samples.iter().map(|x| x.end_virtual).sum::<f64>()
            / size
            / MIB_CONVERSION_FACTOR,
        avg_virtual: samples.iter().map(|x| x.avg_virtual).sum::<f64>()
            / size
            / MIB_CONVERSION_FACTOR,
        min_virtual: samples
            .iter()
            .map(|x| x.min_virtual)
            .fold(f64::INFINITY, f64::min)
            / MIB_CONVERSION_FACTOR,
        max_virtual: samples
            .iter()
            .map(|x| x.max_virtual)
            .fold(f64::NEG_INFINITY, f64::max)
            / MIB_CONVERSION_FACTOR,
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
        if value.is_empty() {
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

pub async fn trace_memory(mut routine: impl FnMut(), duration: Duration) -> Vec<Sample> {
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
}

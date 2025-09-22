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
    pub memory_growth: f64,
    pub peak_over_start_rss_ratio: f64,
    pub avg_rss: f64,
    pub min_rss: f64,
    pub max_rss: f64,
    pub start_virtual: f64,
    pub end_virtual: f64,
    pub virtual_growth: f64,
    pub peak_over_start_virtual_ratio: f64,
    pub avg_virtual: f64,
    pub min_virtual: f64,
    pub max_virtual: f64,
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
                    value: x.memory_growth,
                    ..Default::default()
                },
                peak_over_start_rss_ratio: MetricEntry {
                    value: x.peak_over_start_rss_ratio,
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
                    value: x.virtual_growth,
                    ..Default::default()
                },
                peak_over_start_virtual_ratio: MetricEntry {
                    value: x.peak_over_start_virtual_ratio,
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
        memory_growth: samples.iter().map(|x| x.memory_growth).sum::<f64>()
            / size
            / MIB_CONVERSION_FACTOR,
        peak_over_start_rss_ratio: samples
            .iter()
            .map(|x| x.peak_over_start_rss_ratio)
            .sum::<f64>()
            / size,
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
        virtual_growth: samples.iter().map(|x| x.virtual_growth).sum::<f64>()
            / size
            / MIB_CONVERSION_FACTOR,
        peak_over_start_virtual_ratio: samples
            .iter()
            .map(|x| x.peak_over_start_virtual_ratio)
            .sum::<f64>()
            / size,
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

pub fn get_memory_stats() -> (f64, f64) {
    let mut system = System::new();
    system.refresh_process(Pid::from(std::process::id() as usize));

    system
        .process(Pid::from(std::process::id() as usize))
        .map(|process| (process.memory() as f64, process.virtual_memory() as f64))
        .unwrap_or_default()
}

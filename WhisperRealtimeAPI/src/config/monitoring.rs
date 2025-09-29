use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct MonitoringConfig {
    pub metrics: MetricsExporter,
    pub thresholds: Thresholds,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MetricsExporter {
    pub exporter: String,
    pub listen: String,
    pub scrape_path: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Thresholds {
    pub rtt_ms: ThresholdRange,
    pub jitter_ms: ThresholdRange,
    pub packet_loss_percent: ThresholdRange,
    pub asr_latency_ms: ThresholdRange,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ThresholdRange {
    pub warn: f32,
    pub critical: f32,
}

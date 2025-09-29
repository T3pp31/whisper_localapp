use std::time::Duration;

use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct AsrPipelineConfig {
    pub service: ServiceConfig,
    pub streaming: StreamingConfig,
    pub model: ModelConfig,
}

impl AsrPipelineConfig {
    pub fn request_timeout(&self) -> Duration {
        Duration::from_millis(self.service.request_timeout_ms)
    }

    pub fn partial_result_interval(&self) -> Duration {
        Duration::from_millis(self.streaming.partial_result_interval_ms)
    }

    pub fn finalization_silence(&self) -> Duration {
        Duration::from_millis(self.streaming.finalization_silence_ms)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServiceConfig {
    pub endpoint: String,
    pub request_timeout_ms: u64,
    pub max_stream_duration_s: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct StreamingConfig {
    pub partial_result_interval_ms: u64,
    pub finalization_silence_ms: u64,
    pub max_pending_requests: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ModelConfig {
    pub name: String,
    pub language: String,
    pub enable_vad: bool,
}

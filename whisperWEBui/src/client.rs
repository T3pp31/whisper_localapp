use crate::config::Config;
use reqwest::multipart::{Form, Part};
use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct WhisperClient {
    client: reqwest::Client,
    base_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TranscriptionRequest {
    pub language: Option<String>,
    pub temperature: Option<f32>,
    pub no_speech_threshold: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TranscriptionResponse {
    pub text: String,
    pub language: Option<String>,
    pub duration: Option<f64>,
    pub processing_time: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TimestampedTranscriptionResponse {
    pub text: String,
    pub segments: Vec<Segment>,
    pub language: Option<String>,
    pub duration: Option<f64>,
    pub processing_time: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum BackendTimestampedResponse {
    Full(BackendFullResponse),
    SegmentsOnly(Vec<BackendSegment>),
}

#[derive(Debug, Deserialize)]
struct BackendFullResponse {
    text: String,
    #[serde(default)]
    segments: Vec<BackendSegment>,
    #[serde(default)]
    language: Option<String>,
    #[serde(default)]
    duration: Option<f64>,
    #[serde(rename = "duration_ms", default)]
    duration_ms: Option<f64>,
    #[serde(default)]
    processing_time: Option<f64>,
    #[serde(rename = "processing_time_ms", default)]
    processing_time_ms: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct BackendSegment {
    text: String,
    #[serde(default)]
    start: Option<f64>,
    #[serde(rename = "start_time", default)]
    start_time: Option<f64>,
    #[serde(rename = "start_ms", default)]
    start_ms: Option<f64>,
    #[serde(rename = "start_time_ms", default)]
    start_time_ms: Option<f64>,
    #[serde(default)]
    end: Option<f64>,
    #[serde(rename = "end_time", default)]
    end_time: Option<f64>,
    #[serde(rename = "end_ms", default)]
    end_ms: Option<f64>,
    #[serde(rename = "end_time_ms", default)]
    end_time_ms: Option<f64>,
    #[serde(default)]
    timestamp: Option<Vec<f64>>,
    #[serde(rename = "timestamps", default)]
    timestamps: Option<Vec<f64>>,
}

impl BackendSegment {
    fn into_segment(self) -> Segment {
        let BackendSegment {
            text,
            start,
            start_time,
            start_ms,
            start_time_ms,
            end,
            end_time,
            end_ms,
            end_time_ms,
            timestamp,
            timestamps,
        } = self;

        let bounds_from_array = timestamp
            .as_ref()
            .and_then(|values| extract_bounds(values))
            .or_else(|| timestamps.as_ref().and_then(|values| extract_bounds(values)));

        let start_seconds = bounds_from_array
            .map(|(s, _)| s)
            .or(start)
            .or(start_time)
            .or(start_ms.map(|ms| ms / 1000.0))
            .or(start_time_ms.map(|ms| ms / 1000.0))
            .unwrap_or(0.0);

        let end_seconds = bounds_from_array
            .map(|(_, e)| e)
            .or(end)
            .or(end_time)
            .or(end_ms.map(|ms| ms / 1000.0))
            .or(end_time_ms.map(|ms| ms / 1000.0))
            .unwrap_or(start_seconds);

        Segment {
            start: start_seconds,
            end: end_seconds,
            text,
        }
    }
}

fn extract_bounds(values: &[f64]) -> Option<(f64, f64)> {
    if values.len() >= 2 {
        Some((values[0], values[1]))
    } else {
        None
    }
}

impl TimestampedTranscriptionResponse {
    fn from_backend(raw: BackendTimestampedResponse) -> Self {
        match raw {
            BackendTimestampedResponse::Full(full) => {
                let segments = full
                    .segments
                    .into_iter()
                    .map(BackendSegment::into_segment)
                    .collect::<Vec<_>>();

                let duration = full
                    .duration
                    .or(full.duration_ms.map(|ms| ms / 1000.0))
                    .or_else(|| Self::max_segment_end(&segments));

                let processing_time = full
                    .processing_time
                    .or(full.processing_time_ms.map(|ms| ms / 1000.0));

                TimestampedTranscriptionResponse {
                    text: full.text,
                    segments,
                    language: full.language,
                    duration,
                    processing_time,
                }
            }
            BackendTimestampedResponse::SegmentsOnly(segments_only) => {
                let segments = segments_only
                    .into_iter()
                    .map(BackendSegment::into_segment)
                    .collect::<Vec<_>>();

                let combined_text: String = segments
                    .iter()
                    .map(|segment| segment.text.as_str())
                    .collect();

                let text = if combined_text.is_empty() {
                    combined_text
                } else {
                    combined_text.trim().to_string()
                };

                let duration = Self::max_segment_end(&segments);

                TimestampedTranscriptionResponse {
                    text,
                    segments,
                    language: None,
                    duration,
                    processing_time: None,
                }
            }
        }
    }

    fn max_segment_end(segments: &[Segment]) -> Option<f64> {
        segments
            .iter()
            .map(|segment| segment.end)
            .fold(None, |acc, value| match acc {
                Some(current) => Some(current.max(value)),
                None => Some(value),
            })
    }

    pub fn from_backend_json(payload: &str) -> Result<Self, ClientError> {
        let raw: BackendTimestampedResponse = serde_json::from_str(payload)
            .map_err(|e| ClientError::InvalidResponse(format!("JSONパースエラー: {}", e)))?;

        Ok(Self::from_backend(raw))
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub uptime_seconds: f64,
    pub whisper_loaded: bool,
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct StatsResponse {
    pub requests_total: u64,
    pub requests_successful: u64,
    pub requests_failed: u64,
    pub uptime_seconds: f64,
    pub average_processing_time: Option<f64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelsResponse {
    pub models: Vec<String>,
    pub current_model: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct LanguagesResponse {
    pub languages: Vec<Language>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Language {
    pub code: String,
    pub name: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GpuStatusResponse {
    pub gpu_available: bool,
    pub gpu_name: Option<String>,
    pub gpu_memory_total: Option<u64>,
    pub gpu_memory_used: Option<u64>,
    pub gpu_utilization: Option<f32>,
}

#[derive(Debug)]
pub enum ClientError {
    Network(reqwest::Error),
    InvalidResponse(String),
    ServerError(String),
}

impl From<reqwest::Error> for ClientError {
    fn from(error: reqwest::Error) -> Self {
        ClientError::Network(error)
    }
}

impl std::fmt::Display for ClientError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ClientError::Network(err) => write!(f, "ネットワークエラー: {}", err),
            ClientError::InvalidResponse(msg) => write!(f, "無効なレスポンス: {}", msg),
            ClientError::ServerError(msg) => write!(f, "サーバーエラー: {}", msg),
        }
    }
}

impl std::error::Error for ClientError {}

impl WhisperClient {
    pub fn new(config: &Config) -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.backend.timeout_seconds))
            .build()
            .expect("HTTPクライアントの作成に失敗しました");

        Self {
            client,
            base_url: config.backend.base_url.clone(),
        }
    }

    pub async fn health_check(&self) -> Result<HealthResponse, ClientError> {
        let url = format!("{}/health", self.base_url);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(ClientError::ServerError(format!(
                "HTTP {}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        let health_response: HealthResponse = response.json().await
            .map_err(|e| ClientError::InvalidResponse(format!("JSONパースエラー: {}", e)))?;

        Ok(health_response)
    }

    pub async fn get_stats(&self) -> Result<StatsResponse, ClientError> {
        let url = format!("{}/stats", self.base_url);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(ClientError::ServerError(format!(
                "HTTP {}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        let stats_response: StatsResponse = response.json().await
            .map_err(|e| ClientError::InvalidResponse(format!("JSONパースエラー: {}", e)))?;

        Ok(stats_response)
    }

    pub async fn get_models(&self) -> Result<ModelsResponse, ClientError> {
        let url = format!("{}/models", self.base_url);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(ClientError::ServerError(format!(
                "HTTP {}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        let models_response: ModelsResponse = response.json().await
            .map_err(|e| ClientError::InvalidResponse(format!("JSONパースエラー: {}", e)))?;

        Ok(models_response)
    }

    pub async fn get_languages(&self) -> Result<LanguagesResponse, ClientError> {
        let url = format!("{}/languages", self.base_url);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(ClientError::ServerError(format!(
                "HTTP {}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        let languages_response: LanguagesResponse = response.json().await
            .map_err(|e| ClientError::InvalidResponse(format!("JSONパースエラー: {}", e)))?;

        Ok(languages_response)
    }

    pub async fn get_gpu_status(&self) -> Result<GpuStatusResponse, ClientError> {
        let url = format!("{}/gpu-status", self.base_url);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            return Err(ClientError::ServerError(format!(
                "HTTP {}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        let gpu_status_response: GpuStatusResponse = response.json().await
            .map_err(|e| ClientError::InvalidResponse(format!("JSONパースエラー: {}", e)))?;

        Ok(gpu_status_response)
    }

    pub async fn transcribe(
        &self,
        audio_data: Vec<u8>,
        filename: &str,
        request: &TranscriptionRequest,
    ) -> Result<TranscriptionResponse, ClientError> {
        let url = format!("{}/transcribe", self.base_url);

        let mut form = Form::new()
            .part("file", Part::bytes(audio_data).file_name(filename.to_string()));

        if let Some(ref language) = request.language {
            form = form.text("language", language.clone());
        }
        if let Some(temperature) = request.temperature {
            form = form.text("temperature", temperature.to_string());
        }
        if let Some(threshold) = request.no_speech_threshold {
            form = form.text("no_speech_threshold", threshold.to_string());
        }

        let response = self.client.post(&url).multipart(form).send().await?;

        if !response.status().is_success() {
            return Err(ClientError::ServerError(format!(
                "HTTP {}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        let transcription_response: TranscriptionResponse = response.json().await
            .map_err(|e| ClientError::InvalidResponse(format!("JSONパースエラー: {}", e)))?;

        Ok(transcription_response)
    }

    pub async fn transcribe_with_timestamps(
        &self,
        audio_data: Vec<u8>,
        filename: &str,
        request: &TranscriptionRequest,
    ) -> Result<TimestampedTranscriptionResponse, ClientError> {
        let url = format!("{}/transcribe-with-timestamps", self.base_url);

        let mut form = Form::new()
            .part("file", Part::bytes(audio_data).file_name(filename.to_string()));

        if let Some(ref language) = request.language {
            form = form.text("language", language.clone());
        }
        if let Some(temperature) = request.temperature {
            form = form.text("temperature", temperature.to_string());
        }
        if let Some(threshold) = request.no_speech_threshold {
            form = form.text("no_speech_threshold", threshold.to_string());
        }

        let response = self.client.post(&url).multipart(form).send().await?;

        if !response.status().is_success() {
            return Err(ClientError::ServerError(format!(
                "HTTP {}: {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        let body = response.text().await?;
        let transcription_response = TimestampedTranscriptionResponse::from_backend_json(&body)?;

        Ok(transcription_response)
    }
}

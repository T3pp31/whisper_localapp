use crate::client::{
    GpuStatusResponse,
    HealthResponse,
    StatsResponse,
    TranscriptionRequest,
    WhisperClient,
};
use crate::config::Config;
use axum::{
    extract::{Multipart, State},
    response::{Html, Json},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub client: WhisperClient,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let client = WhisperClient::new(&config);
        Self {
            client,
            config: Arc::new(config),
        }
    }
}

fn encode_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

#[derive(Debug, Deserialize)]
pub struct UploadForm {
    pub language: Option<String>,
    pub with_timestamps: Option<bool>,
    pub temperature: Option<f32>,
    pub no_speech_threshold: Option<f32>,
}

#[derive(Debug, Serialize)]
pub struct UploadResponse {
    pub success: bool,
    pub message: String,
    pub data: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub success: bool,
    pub error: String,
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct FrontendHealth {
    pub status: String,
    pub version: Option<String>,
    pub whisper_loaded: bool,
    pub uptime_seconds: u64,
    pub memory_usage_mb: Option<u64>,
}

pub fn map_health_response(health: HealthResponse) -> FrontendHealth {
    FrontendHealth {
        status: health.status,
        version: health.version.and_then(|v| {
            let trimmed = v.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }),
        whisper_loaded: health.model_loaded,
        uptime_seconds: health.uptime_seconds,
        memory_usage_mb: health.memory_usage_mb,
    }
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct FrontendStats {
    pub requests_total: u64,
    pub requests_successful: u64,
    pub requests_failed: u64,
    pub uptime_seconds: u64,
    pub average_processing_time: Option<f64>,
    pub active_requests: usize,
}

pub fn map_stats_response(stats: StatsResponse) -> FrontendStats {
    let average_processing_time = if stats.successful_transcriptions > 0 {
        Some(stats.average_processing_time_ms / 1000.0)
    } else {
        None
    };

    FrontendStats {
        requests_total: stats.total_requests,
        requests_successful: stats.successful_transcriptions,
        requests_failed: stats.failed_transcriptions,
        uptime_seconds: stats.uptime_seconds,
        average_processing_time,
        active_requests: stats.active_requests,
    }
}

#[derive(Debug, Serialize, Clone, PartialEq)]
pub struct FrontendGpuStatus {
    pub gpu_available: bool,
    pub gpu_name: Option<String>,
    pub gpu_enabled_in_config: bool,
}

pub fn map_gpu_status_response(status: GpuStatusResponse) -> FrontendGpuStatus {
    let gpu_available = if status.gpu_actually_enabled {
        true
    } else {
        status
            .model_info
            .as_ref()
            .map(|info| info.enable_gpu && info.is_loaded)
            .unwrap_or(false)
    };

    let gpu_name = if gpu_available {
        Some("GPU".to_string())
    } else {
        None
    };

    FrontendGpuStatus {
        gpu_available,
        gpu_name,
        gpu_enabled_in_config: status.gpu_enabled_in_config,
    }
}

pub async fn index(State(state): State<AppState>) -> Html<String> {
    let allowed_exts = state.config.webui.allowed_extensions.join(", ");
    let accept_types = state
        .config
        .webui
        .allowed_extensions
        .iter()
        .map(|ext| format!(".{}", ext))
        .collect::<Vec<_>>()
        .join(",");
    let default_language = state
        .config
        .webui
        .default_language
        .clone()
        .unwrap_or_default();
    let timeline_update_ms = state.config.webui.timeline_update_interval_ms;
    let upload_prompt_text = encode_html(&state.config.webui.upload_prompt_text);
    let upload_success_text = encode_html(&state.config.webui.upload_success_text);

    let html = format!(
        r#"
<!DOCTYPE html>
<html lang="ja">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{}</title>
    <link rel="stylesheet" href="/static/css/style.css">
</head>
<body>
    <div id="app-config" data-default-language="{}" data-timeline-update-ms="{}" style="display: none;"></div>
    <div class="container">
        <header>
            <h1>{}</h1>
            <div class="status-panel">
                <div class="status-item">
                    <span class="status-label">バックエンド:</span>
                    <span id="backend-status" class="status-value">確認中...</span>
                </div>
                <div class="status-item">
                    <span class="status-label">GPU:</span>
                    <span id="gpu-status" class="status-value">確認中...</span>
                </div>
            </div>
        </header>

        <main>
            <div class="upload-section">
                <div class="upload-area" id="upload-area">
                    <div class="upload-content">
                        <div class="upload-icon" aria-hidden="true">📁</div>
                        <p class="upload-text" id="upload-text" data-default-text="{}">{}</p>
                        <p class="upload-status" id="upload-status" data-success-text="{}" aria-live="polite" style="display: none;"></p>
                        <p class="upload-info">対応形式: {} (最大 {}MB)</p>
                        <div class="upload-preview" id="upload-preview" style="display: none;">
                            <audio id="upload-audio-preview" controls></audio>
                        </div>
                        <input type="file" id="file-input" accept="{}" hidden>
                    </div>
                    <div class="upload-progress" id="upload-progress" style="display: none;">
                        <div class="progress-bar">
                            <div class="progress-fill" id="progress-fill"></div>
                        </div>
                        <p class="progress-text" id="progress-text">アップロード中...</p>
                    </div>
                </div>

                <div class="options-panel">
                    <div class="option-group">
                        <label for="language-select">言語:</label>
                        <select id="language-select">
                            <option value="">自動検出</option>
                        </select>
                    </div>

                    <div class="option-group">
                        <label>
                            <input type="checkbox" id="with-timestamps">
                            タイムスタンプを含める
                        </label>
                    </div>

                    <div class="option-group">
                        <label for="temperature">温度 (0.0-1.0):</label>
                        <input type="number" id="temperature" min="0" max="1" step="0.1" placeholder="0.0">
                    </div>

                    <div class="option-group">
                        <label for="no-speech-threshold">無音閾値 (0.0-1.0):</label>
                        <input type="number" id="no-speech-threshold" min="0" max="1" step="0.1" placeholder="0.6">
                    </div>

                    <div class="option-group action-group">
                        <button id="transcribe-btn" class="btn btn-primary" type="button" disabled data-label="文字起こしを開始" data-loading-label="文字起こし中...">文字起こしを開始</button>
                    </div>
                </div>
            </div>

            <div class="results-section" id="results-section" style="display: none;">
                <h2>文字起こし結果</h2>
                <div class="results-controls">
                    <button id="copy-text-btn" class="btn btn-secondary">テキストをコピー</button>
                    <button id="download-text-btn" class="btn btn-secondary">テキストファイルをダウンロード</button>
                    <button id="download-json-btn" class="btn btn-secondary">JSONファイルをダウンロード</button>
                    <button id="clear-results-btn" class="btn btn-danger">結果をクリア</button>
                </div>

                <div class="results-content">
                    <div class="audio-player" id="audio-player-container" style="display: none;">
                        <audio id="audio-player" controls></audio>
                        <div class="timeline-container" id="timeline-container">
                            <div class="timeline" id="timeline">
                                <div class="timeline-progress" id="timeline-progress"></div>
                                <div class="timeline-segments" id="timeline-segments"></div>
                            </div>
                        </div>
                    </div>

                    <div class="result-info">
                        <span class="info-item">処理時間: <span id="processing-time">-</span>秒</span>
                        <span class="info-item">音声長: <span id="audio-duration">-</span>秒</span>
                        <span class="info-item">検出言語: <span id="detected-language">-</span></span>
                    </div>

                    <div class="result-text" id="result-text"></div>

                    <div class="segments-container" id="segments-container" style="display: none;">
                        <h3>タイムスタンプ付きセグメント</h3>
                        <div class="segments" id="segments"></div>
                    </div>
                </div>
            </div>
        </main>

        <footer>
            <div class="server-info">
                <div class="info-panel">
                    <h3>サーバー情報</h3>
                    <div class="info-content" id="server-info">読み込み中...</div>
                </div>
                <div class="info-panel">
                    <h3>統計情報</h3>
                    <div class="info-content" id="stats-info">読み込み中...</div>
                </div>
            </div>
        </footer>
    </div>

    <div class="notification" id="notification" style="display: none;">
        <span class="notification-text" id="notification-text"></span>
        <button class="notification-close" id="notification-close">×</button>
    </div>

    <script src="/static/js/app.js"></script>
</body>
</html>
"#,
        state.config.webui.title,
        default_language,
        timeline_update_ms,
        state.config.webui.title,
        upload_prompt_text,
        upload_prompt_text,
        upload_success_text,
        allowed_exts,
        state.config.webui.max_file_size_mb,
        accept_types
    );

    Html(html)
}

pub async fn upload_file(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> Result<Json<UploadResponse>, Json<ErrorResponse>> {
    let mut file_data: Option<Vec<u8>> = None;
    let mut filename: Option<String> = None;
    let mut language: Option<String> = None;
    let mut with_timestamps: bool = false;
    let mut temperature: Option<f32> = None;
    let mut no_speech_threshold: Option<f32> = None;

    while let Some(field) = multipart.next_field().await.map_err(|e| {
        Json(ErrorResponse {
            success: false,
            error: format!("マルチパートデータの読み込みエラー: {}", e),
        })
    })? {
        let field_name = field.name().unwrap_or("").to_string();

        match field_name.as_str() {
            "file" => {
                filename = field.file_name().map(|s| s.to_string());
                let data = field.bytes().await.map_err(|e| {
                    Json(ErrorResponse {
                        success: false,
                        error: format!("ファイルデータの読み込みエラー: {}", e),
                    })
                })?;

                if data.len() > state.config.max_file_size_bytes() {
                    return Err(Json(ErrorResponse {
                        success: false,
                        error: format!(
                            "ファイルサイズが制限を超えています (最大: {}MB)",
                            state.config.webui.max_file_size_mb
                        ),
                    }));
                }

                file_data = Some(data.to_vec());
            }
            "language" => {
                let value = field.text().await.map_err(|e| {
                    Json(ErrorResponse {
                        success: false,
                        error: format!("言語パラメータの読み込みエラー: {}", e),
                    })
                })?;
                if !value.is_empty() {
                    language = Some(value);
                }
            }
            "with_timestamps" => {
                let value = field.text().await.map_err(|e| {
                    Json(ErrorResponse {
                        success: false,
                        error: format!("タイムスタンプパラメータの読み込みエラー: {}", e),
                    })
                })?;
                with_timestamps = value == "true" || value == "1";
            }
            "temperature" => {
                let value = field.text().await.map_err(|e| {
                    Json(ErrorResponse {
                        success: false,
                        error: format!("温度パラメータの読み込みエラー: {}", e),
                    })
                })?;
                if !value.is_empty() {
                    temperature = value.parse().ok();
                }
            }
            "no_speech_threshold" => {
                let value = field.text().await.map_err(|e| {
                    Json(ErrorResponse {
                        success: false,
                        error: format!("無音閾値パラメータの読み込みエラー: {}", e),
                    })
                })?;
                if !value.is_empty() {
                    no_speech_threshold = value.parse().ok();
                }
            }
            _ => {}
        }
    }

    let file_data = file_data.ok_or_else(|| {
        Json(ErrorResponse {
            success: false,
            error: "ファイルが指定されていません".to_string(),
        })
    })?;

    let filename = filename.ok_or_else(|| {
        Json(ErrorResponse {
            success: false,
            error: "ファイル名が指定されていません".to_string(),
        })
    })?;

    if let Some(ext) = filename.split('.').last() {
        if !state.config.is_allowed_extension(ext) {
            return Err(Json(ErrorResponse {
                success: false,
                error: format!(
                    "サポートされていないファイル形式です。許可されている形式: {}",
                    state.config.webui.allowed_extensions.join(", ")
                ),
            }));
        }
    }

    let request = TranscriptionRequest {
        language,
        temperature,
        no_speech_threshold,
    };

    let result = if with_timestamps {
        state
            .client
            .transcribe_with_timestamps(file_data, &filename, &request)
            .await
            .map(|response| json!(response))
    } else {
        state
            .client
            .transcribe(file_data, &filename, &request)
            .await
            .map(|response| json!(response))
    };

    match result {
        Ok(data) => Ok(Json(UploadResponse {
            success: true,
            message: "文字起こしが完了しました".to_string(),
            data: Some(data),
        })),
        Err(e) => Err(Json(ErrorResponse {
            success: false,
            error: format!("文字起こしエラー: {}", e),
        })),
    }
}

pub async fn backend_health(State(state): State<AppState>) -> Json<serde_json::Value> {
    match state.client.health_check().await {
        Ok(health) => {
            let mapped = map_health_response(health);
            Json(json!({
                "success": true,
                "data": mapped
            }))
        },
        Err(e) => Json(json!({
            "success": false,
            "error": e.to_string()
        })),
    }
}

pub async fn backend_stats(State(state): State<AppState>) -> Json<serde_json::Value> {
    match state.client.get_stats().await {
        Ok(stats) => {
            let mapped = map_stats_response(stats);
            Json(json!({
                "success": true,
                "data": mapped
            }))
        },
        Err(e) => Json(json!({
            "success": false,
            "error": e.to_string()
        })),
    }
}

pub async fn backend_models(State(state): State<AppState>) -> Json<serde_json::Value> {
    match state.client.get_models().await {
        Ok(models) => Json(json!({
            "success": true,
            "data": models
        })),
        Err(e) => Json(json!({
            "success": false,
            "error": e.to_string()
        })),
    }
}

pub async fn backend_languages(State(state): State<AppState>) -> Json<serde_json::Value> {
    match state.client.get_languages().await {
        Ok(languages_response) => Json(json!({
            "success": true,
            "data": {
                "languages": languages_response.languages
            }
        })),
        Err(e) => Json(json!({
            "success": false,
            "error": e.to_string()
        })),
    }
}

pub async fn backend_gpu_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    match state.client.get_gpu_status().await {
        Ok(gpu_status) => {
            let mapped = map_gpu_status_response(gpu_status);
            Json(json!({
                "success": true,
                "data": mapped
            }))
        },
        Err(e) => Json(json!({
            "success": false,
            "error": e.to_string()
        })),
    }
}

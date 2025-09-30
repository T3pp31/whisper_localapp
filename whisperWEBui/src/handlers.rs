use crate::client::{
    GpuStatusResponse, HealthResponse, StatsResponse, TranscriptionRequest, WhisperClient,
};
use crate::config::{Config, RealtimeConfig};
use axum::{
    extract::{Multipart, Path, State},
    response::{Html, Json},
};
use log::error;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use whisper_realtime_api::config::{ConfigSet, IceServerConfig};
use whisper_realtime_api::signaling::{
    ClientMetadata, ClientType, IceServer as SignalingIceServer, NoopTokenValidator,
    SessionRequest, SignalingError, SignalingService,
};

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub client: WhisperClient,
    pub realtime: Option<RealtimeState>,
}

#[derive(Clone)]
pub struct RealtimeState {
    pub web_config: RealtimeConfig,
    pub config_set: Arc<ConfigSet>,
    pub signaling: SignalingService<NoopTokenValidator>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        let realtime = RealtimeState::initialize(&config);
        let client = WhisperClient::new(&config);

        Self {
            client,
            config: Arc::new(config),
            realtime,
        }
    }
}

impl RealtimeState {
    fn initialize(config: &Config) -> Option<Self> {
        if !config.realtime.enabled {
            return None;
        }

        let Some(config_dir) = config.realtime.config_dir_path() else {
            error!("リアルタイム設定のconfig_dirが指定されていません");
            return None;
        };

        let config_set = match ConfigSet::load_from_dir(&config_dir) {
            Ok(value) => Arc::new(value),
            Err(err) => {
                error!(
                    "リアルタイム設定のロードに失敗しました ({}): {}",
                    config_dir.display(),
                    err
                );
                return None;
            }
        };

        let signaling = SignalingService::with_default_validator(config_set.clone());

        Some(Self {
            web_config: config.realtime.clone(),
            config_set,
            signaling,
        })
    }

    fn audience(&self) -> &str {
        &self.config_set.system.token.audience
    }

    fn default_client_type(&self) -> Option<&str> {
        self.web_config
            .default_client_type
            .as_deref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
    }

    fn default_client_name(&self) -> Option<&str> {
        self.web_config
            .default_client_name
            .as_deref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
    }

    fn default_client_version(&self) -> Option<&str> {
        self.web_config
            .default_client_version
            .as_deref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
    }

    fn default_token_subject(&self) -> Option<&str> {
        self.web_config
            .default_token_subject
            .as_deref()
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
    }

    fn heartbeat_interval_ms(&self) -> u64 {
        self.web_config.heartbeat_interval_ms
    }
}

fn encode_html(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('\"', "&quot;")
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
    pub model_name: Option<String>,
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
        model_name: health.model_name.and_then(|m| {
            let trimmed = m.trim();
            if trimmed.is_empty() {
                None
            } else {
                Some(trimmed.to_string())
            }
        }),
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

#[derive(Debug, Deserialize)]
pub struct RealtimeSessionStartRequest {
    pub client_type: String,
    pub client_name: String,
    pub client_version: String,
    pub token_subject: String,
    #[serde(default)]
    pub retry: bool,
}

#[derive(Debug, Serialize)]
pub struct RealtimeSessionStartResponse {
    pub session_id: String,
    pub ice_servers: Vec<RealtimeIceServer>,
    pub max_bitrate_kbps: u32,
}

#[derive(Debug, Serialize)]
pub struct RealtimeIceServer {
    pub urls: Vec<String>,
    pub username: Option<String>,
    pub credential: Option<String>,
}

impl From<SignalingIceServer> for RealtimeIceServer {
    fn from(value: SignalingIceServer) -> Self {
        Self {
            urls: value.urls,
            username: value.username,
            credential: value.credential,
        }
    }
}

impl From<&IceServerConfig> for RealtimeIceServer {
    fn from(value: &IceServerConfig) -> Self {
        Self {
            urls: value.urls.clone(),
            username: value.username.clone().filter(|s| !s.trim().is_empty()),
            credential: value.credential.clone().filter(|s| !s.trim().is_empty()),
        }
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
    let stats_average_label = encode_html(&state.config.webui.stats_average_processing_time_label);
    let stats_average_unit = encode_html(&state.config.webui.stats_average_processing_time_unit);

    let realtime_config = &state.config.realtime;
    let realtime_enabled_attr = if realtime_config.enabled {
        "true"
    } else {
        "false"
    };
    let realtime_client_type =
        encode_html(realtime_config.default_client_type.as_deref().unwrap_or(""));
    let realtime_client_name =
        encode_html(realtime_config.default_client_name.as_deref().unwrap_or(""));
    let realtime_client_version = encode_html(
        realtime_config
            .default_client_version
            .as_deref()
            .unwrap_or(""),
    );
    let realtime_token_subject = encode_html(
        realtime_config
            .default_token_subject
            .as_deref()
            .unwrap_or(""),
    );
    let realtime_heartbeat_ms = realtime_config.heartbeat_interval_ms;

    let html = format!(
        r#"
<!DOCTYPE html>
<html lang="ja">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{title}</title>
    <link rel="stylesheet" href="/static/css/style.css">
</head>
<body>
    <div id="app-config"
         data-default-language="{default_language}"
         data-timeline-update-ms="{timeline_ms}"
         data-stats-average-processing-time-label="{stats_label}"
         data-stats-average-processing-time-unit="{stats_unit}"
         data-realtime-enabled="{realtime_enabled}"
         data-realtime-client-type="{realtime_client_type}"
         data-realtime-client-name="{realtime_client_name}"
         data-realtime-client-version="{realtime_client_version}"
         data-realtime-token-subject="{realtime_token_subject}"
         data-realtime-heartbeat-ms="{realtime_heartbeat_ms}"
         style="display: none;"></div>
    <div class="container">
        <header>
            <h1>{title}</h1>
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
            <nav class="tab-bar" role="tablist" aria-label="機能タブ">
                <button class="tab-button active" type="button" data-tab="batch" id="tab-batch" aria-selected="true" aria-controls="panel-batch">ファイル文字起こし</button>
                <button class="tab-button" type="button" data-tab="realtime" id="tab-realtime" aria-selected="false" aria-controls="panel-realtime">リアルタイム</button>
            </nav>
            <div class="tab-panels">
                <section class="tab-panel active" data-tab-panel="batch" id="panel-batch" role="tabpanel" aria-labelledby="tab-batch">
                    <div class="upload-section">
                        <div class="upload-area" id="upload-area">
                            <div class="upload-content">
                                <div class="upload-icon" aria-hidden="true">📁</div>
                                <p class="upload-text" id="upload-text" data-default-text="{upload_prompt}">{upload_prompt}</p>
                                <p class="upload-status" id="upload-status" data-success-text="{upload_success}" aria-live="polite" style="display: none;"></p>
                                <p class="upload-info">対応形式: {allowed_exts} (最大 {max_size}MB)</p>
                                <div class="upload-preview" id="upload-preview" style="display: none;">
                                    <audio id="upload-audio-preview" controls></audio>
                                </div>
                                <!--
                                    互換性のため、hidden属性での非表示クリックは避け、
                                    透明オーバーレイのfile inputでクリック/ドロップの両方を拾う
                                -->
                                <input
                                    type="file"
                                    id="file-input"
                                    class="file-input-overlay"
                                    accept="{accept_types}"
                                    aria-label="音声ファイルを選択"
                                >
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
                </section>

                <section class="tab-panel" data-tab-panel="realtime" id="panel-realtime" role="tabpanel" aria-labelledby="tab-realtime">
                    <div class="realtime-overview">
                        <div class="realtime-status-card">
                            <h2>リアルタイム接続状況</h2>
                            <div class="realtime-metrics">
                                <div>利用可否: <span id="realtime-availability">確認中...</span></div>
                                <div>セッション数: <span id="realtime-active-sessions">-</span> / <span id="realtime-max-sessions">-</span></div>
                                <div>推奨ビットレート: <span id="realtime-max-bitrate">-</span> kbps</div>
                            </div>
                        </div>
                        <div class="realtime-actions">
                            <div class="option-group">
                                <label for="realtime-client-type">クライアント種別</label>
                                <select id="realtime-client-type">
                                    <option value="browser">ブラウザ</option>
                                    <option value="mobile">モバイル</option>
                                </select>
                            </div>
                            <div class="option-group">
                                <label for="realtime-client-name">クライアント名</label>
                                <input type="text" id="realtime-client-name" placeholder="例: Chrome">
                            </div>
                            <div class="option-group">
                                <label for="realtime-client-version">バージョン</label>
                                <input type="text" id="realtime-client-version" placeholder="例: 120">
                            </div>
                            <div class="option-group">
                                <label for="realtime-token-subject">トークン識別子</label>
                                <input type="text" id="realtime-token-subject" placeholder="ユーザーIDなど">
                            </div>
                            <div class="option-group checkbox-group">
                                <label>
                                    <input type="checkbox" id="realtime-retry">
                                    既存セッションへ再接続を試みる
                                </label>
                            </div>
                            <div class="option-group action-group realtime-actions-buttons">
                                <button id="realtime-start-btn" class="btn btn-primary" type="button">セッションを開始</button>
                                <button id="realtime-heartbeat-btn" class="btn btn-secondary" type="button" disabled>ハートビート送信</button>
                                <button id="realtime-end-btn" class="btn btn-danger" type="button" disabled>セッションを終了</button>
                            </div>
                        </div>
                    </div>
                    <div class="realtime-session-info" id="realtime-session-info" style="display: none;">
                        <h3>現在のセッション</h3>
                        <div class="realtime-session-details">
                            <div>セッションID: <span id="realtime-session-id">-</span></div>
                            <div class="realtime-ice">
                                <span>ICEサーバー:</span>
                                <pre id="realtime-ice-servers"></pre>
                            </div>
                        </div>
                    </div>
                    <div class="realtime-log" id="realtime-log" aria-live="polite"></div>
                </section>
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

    <script src="/static/js/realtime-webrtc.js"></script>
    <script src="/static/js/app.js"></script>
</body>
</html>
"#,
        title = encode_html(&state.config.webui.title),
        default_language = encode_html(&default_language),
        timeline_ms = timeline_update_ms,
        stats_label = stats_average_label,
        stats_unit = stats_average_unit,
        realtime_enabled = realtime_enabled_attr,
        realtime_client_type = realtime_client_type,
        realtime_client_name = realtime_client_name,
        realtime_client_version = realtime_client_version,
        realtime_token_subject = realtime_token_subject,
        realtime_heartbeat_ms = realtime_heartbeat_ms,
        upload_prompt = upload_prompt_text,
        upload_success = upload_success_text,
        allowed_exts = encode_html(&allowed_exts),
        max_size = state.config.webui.max_file_size_mb,
        accept_types = encode_html(&accept_types),
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
        }
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
        }
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
        }
        Err(e) => Json(json!({
            "success": false,
            "error": e.to_string()
        })),
    }
}

pub async fn realtime_config(State(state): State<AppState>) -> Json<serde_json::Value> {
    if let Some(realtime) = &state.realtime {
        let active_sessions = realtime.signaling.active_sessions().await;
        let max_sessions = realtime.config_set.system.resources.max_concurrent_sessions;
        let ice_servers: Vec<RealtimeIceServer> = realtime
            .config_set
            .system
            .signaling
            .ice_servers
            .iter()
            .map(RealtimeIceServer::from)
            .collect();

        Json(json!({
            "success": true,
            "data": {
                "enabled": true,
                "audience": realtime.audience(),
                "default_client_type": realtime.default_client_type(),
                "default_client_name": realtime.default_client_name(),
                "default_client_version": realtime.default_client_version(),
                "default_token_subject": realtime.default_token_subject(),
                "heartbeat_interval_ms": realtime.heartbeat_interval_ms(),
                "active_sessions": active_sessions,
                "max_sessions": max_sessions,
                "max_bitrate_kbps": realtime.config_set.system.signaling.default_bitrate_kbps,
                "ice_servers": ice_servers,
            }
        }))
    } else {
        Json(json!({
            "success": true,
            "data": {
                "enabled": false
            }
        }))
    }
}

pub async fn realtime_start_session(
    State(state): State<AppState>,
    Json(payload): Json<RealtimeSessionStartRequest>,
) -> Result<Json<serde_json::Value>, Json<ErrorResponse>> {
    let Some(realtime) = &state.realtime else {
        return Err(Json(ErrorResponse {
            success: false,
            error: "リアルタイムバックエンドは無効です".to_string(),
        }));
    };

    let client_type = parse_client_type(&payload.client_type).ok_or_else(|| {
        Json(ErrorResponse {
            success: false,
            error: "クライアント種別は browser または mobile を指定してください".to_string(),
        })
    })?;

    let client_name = payload.client_name.trim();
    if client_name.is_empty() {
        return Err(Json(ErrorResponse {
            success: false,
            error: "クライアント名を入力してください".to_string(),
        }));
    }

    let client_version = payload.client_version.trim();
    if client_version.is_empty() {
        return Err(Json(ErrorResponse {
            success: false,
            error: "クライアントのバージョンを入力してください".to_string(),
        }));
    }

    let token_subject = payload.token_subject.trim();
    if token_subject.is_empty() {
        return Err(Json(ErrorResponse {
            success: false,
            error: "トークン識別子を入力してください".to_string(),
        }));
    }

    let metadata = make_client_metadata(client_type, client_name, client_version);
    let auth_token = format!("{}:{}", realtime.audience(), token_subject);

    let request = SessionRequest {
        client: metadata,
        auth_token,
        retry: payload.retry,
    };

    match realtime.signaling.start_session(request).await {
        Ok(response) => {
            let payload = RealtimeSessionStartResponse {
                session_id: response.session_id,
                ice_servers: response
                    .ice_servers
                    .into_iter()
                    .map(RealtimeIceServer::from)
                    .collect(),
                max_bitrate_kbps: response.max_bitrate_kbps,
            };

            Ok(Json(json!({
                "success": true,
                "data": payload
            })))
        }
        Err(err) => Err(Json(ErrorResponse {
            success: false,
            error: map_signaling_error(err),
        })),
    }
}

pub async fn realtime_heartbeat(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Json<serde_json::Value> {
    if let Some(realtime) = &state.realtime {
        match realtime.signaling.heartbeat(&session_id).await {
            Ok(_) => Json(json!({
                "success": true,
                "message": "ハートビートを送信しました"
            })),
            Err(err) => Json(json!({
                "success": false,
                "error": map_signaling_error(err)
            })),
        }
    } else {
        Json(json!({
            "success": false,
            "error": "リアルタイムバックエンドは無効です"
        }))
    }
}

pub async fn realtime_end_session(
    State(state): State<AppState>,
    Path(session_id): Path<String>,
) -> Json<serde_json::Value> {
    if let Some(realtime) = &state.realtime {
        match realtime.signaling.end_session(&session_id).await {
            Ok(_) => Json(json!({
                "success": true,
                "message": "セッションを終了しました"
            })),
            Err(err) => Json(json!({
                "success": false,
                "error": map_signaling_error(err)
            })),
        }
    } else {
        Json(json!({
            "success": false,
            "error": "リアルタイムバックエンドは無効です"
        }))
    }
}

fn parse_client_type(value: &str) -> Option<ClientType> {
    match value.trim().to_ascii_lowercase().as_str() {
        "browser" => Some(ClientType::Browser),
        "mobile" => Some(ClientType::Mobile),
        _ => None,
    }
}

fn make_client_metadata(client_type: ClientType, name: &str, version: &str) -> ClientMetadata {
    match client_type {
        ClientType::Browser => ClientMetadata::browser(name.to_string(), version.to_string()),
        ClientType::Mobile => ClientMetadata::mobile(name.to_string(), version.to_string()),
    }
}

fn map_signaling_error(error: SignalingError) -> String {
    match error {
        SignalingError::Authentication { reason } => {
            format!("認証に失敗しました: {}", reason)
        }
        SignalingError::ClientNotSupported { reason } => {
            format!("サポートされていないクライアントです: {}", reason)
        }
        SignalingError::ResourceLimitExceeded => "セッション数の上限に達しました".to_string(),
        SignalingError::SessionNotFound { session_id } => {
            format!("セッションが見つかりません: {}", session_id)
        }
        SignalingError::Internal { message } => {
            format!("内部エラーが発生しました: {}", message)
        }
    }
}

/// WebSocketシグナリングハンドラ
pub async fn websocket_handler(
    ws: axum::extract::ws::WebSocketUpgrade,
    axum::extract::Path(session_id): axum::extract::Path<String>,
    State(state): State<AppState>,
) -> axum::response::Response {
    log::info!("WebSocket接続リクエスト: session_id={}", session_id);

    ws.on_upgrade(move |socket| handle_websocket(socket, session_id, state))
}

async fn handle_websocket(
    socket: axum::extract::ws::WebSocket,
    session_id: String,
    state: AppState,
) {
    use axum::extract::ws::Message;
    use futures_util::{SinkExt, StreamExt};

    log::info!("WebSocket接続確立: session_id={}", session_id);

    let (mut sender, mut receiver) = socket.split();

    // 簡易的なエコーサーバー実装（実際はWebRTCシグナリングロジックを統合）
    while let Some(msg) = receiver.next().await {
        match msg {
            Ok(Message::Text(text)) => {
                log::debug!("受信メッセージ: {}", text);

                // TODO: ここでWebRTCトランスポートと統合
                // - Offerを受信 → Answerを生成
                // - ICE Candidateを処理

                let response = serde_json::json!({
                    "type": "ack",
                    "session_id": session_id,
                    "message": "received"
                });

                if sender
                    .send(Message::Text(serde_json::to_string(&response).unwrap().into()))
                    .await
                    .is_err()
                {
                    log::warn!("WebSocket送信失敗: session_id={}", session_id);
                    break;
                }
            }
            Ok(Message::Close(_)) => {
                log::info!("WebSocket切断: session_id={}", session_id);
                break;
            }
            Err(e) => {
                log::error!("WebSocketエラー: {}, session_id={}", e, session_id);
                break;
            }
            _ => {}
        }
    }

    log::info!("WebSocket接続終了: session_id={}", session_id);
}

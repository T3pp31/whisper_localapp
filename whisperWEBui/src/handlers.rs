use crate::client::{WhisperClient, TranscriptionRequest};
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

pub async fn index(State(state): State<AppState>) -> Html<String> {
    let allowed_exts = state.config.webui.allowed_extensions.join(", ");
    let accept_types = state.config.webui.allowed_extensions
        .iter()
        .map(|ext| format!(".{}", ext))
        .collect::<Vec<_>>()
        .join(",");

    let html = format!(r#"
<!DOCTYPE html>
<html lang="ja">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>{}</title>
    <link rel="stylesheet" href="/static/css/style.css">
</head>
<body>
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
                        <div class="upload-icon">📁</div>
                        <p class="upload-text">音声ファイルをドラッグ&ドロップするか、クリックして選択してください</p>
                        <p class="upload-info">対応形式: {} (最大 {}MB)</p>
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
        state.config.webui.title,
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
        state.client.transcribe_with_timestamps(file_data, &filename, &request).await
            .map(|response| json!(response))
    } else {
        state.client.transcribe(file_data, &filename, &request).await
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
        Ok(health) => Json(json!({
            "success": true,
            "data": health
        })),
        Err(e) => Json(json!({
            "success": false,
            "error": e.to_string()
        })),
    }
}

pub async fn backend_stats(State(state): State<AppState>) -> Json<serde_json::Value> {
    match state.client.get_stats().await {
        Ok(stats) => Json(json!({
            "success": true,
            "data": stats
        })),
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
        Ok(languages) => Json(json!({
            "success": true,
            "data": languages
        })),
        Err(e) => Json(json!({
            "success": false,
            "error": e.to_string()
        })),
    }
}

pub async fn backend_gpu_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    match state.client.get_gpu_status().await {
        Ok(gpu_status) => Json(json!({
            "success": true,
            "data": gpu_status
        })),
        Err(e) => Json(json!({
            "success": false,
            "error": e.to_string()
        })),
    }
}
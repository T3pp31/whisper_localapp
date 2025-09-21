use crate::audio::{AudioProcessor, format_file_size};
use crate::config::Config;
use crate::models::*;
use crate::whisper::{WhisperEngine, get_supported_languages, get_language_name, preprocess_audio};
use axum::{
    extract::{Multipart, State},
    http::StatusCode,
    response::Json,
};
use std::sync::{Arc, Mutex};
use std::time::Instant;

// =============================================================================
// Application State
// =============================================================================

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub whisper_engine: Arc<Mutex<Option<WhisperEngine>>>,
    pub stats: Arc<Mutex<ServerStats>>,
    pub start_time: Arc<Instant>,
}

impl AppState {
    pub fn new(config: Config) -> Self {
        Self {
            config: Arc::new(config),
            whisper_engine: Arc::new(Mutex::new(None)),
            stats: Arc::new(Mutex::new(ServerStats::default())),
            start_time: Arc::new(Instant::now()),
        }
    }

    pub fn with_whisper_engine(self, engine: WhisperEngine) -> Self {
        *self.whisper_engine.lock().unwrap() = Some(engine);
        self
    }
}

// =============================================================================
// Error Handling
// =============================================================================

pub type ApiResult<T> = Result<T, ApiError>;

#[derive(Debug)]
pub struct ApiError {
    pub code: ApiErrorCode,
    pub message: String,
    pub details: Option<String>,
}

impl ApiError {
    pub fn new(code: ApiErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(err: anyhow::Error) -> Self {
        ApiError::new(ApiErrorCode::InternalError, err.to_string())
    }
}

impl axum::response::IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let status_code = match self.code {
            ApiErrorCode::InvalidInput => StatusCode::BAD_REQUEST,
            ApiErrorCode::FileTooLarge => StatusCode::PAYLOAD_TOO_LARGE,
            ApiErrorCode::UnsupportedFormat => StatusCode::UNSUPPORTED_MEDIA_TYPE,
            ApiErrorCode::ProcessingFailed => StatusCode::INTERNAL_SERVER_ERROR,
            ApiErrorCode::ModelNotLoaded => StatusCode::SERVICE_UNAVAILABLE,
            ApiErrorCode::ServerOverloaded => StatusCode::TOO_MANY_REQUESTS,
            ApiErrorCode::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
        };

        let response = ErrorResponse {
            error: self.message,
            code: self.code.as_str().to_string(),
            details: self.details,
        };

        (status_code, Json(response)).into_response()
    }
}

// =============================================================================
// Request Handlers
// =============================================================================

/// 基本的な文字起こしエンドポイント
pub async fn transcribe_basic(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> ApiResult<Json<TranscribeResponse>> {
    // 統計情報を更新
    {
        let mut stats = state.stats.lock().unwrap();
        stats.record_request();
    }

    let start_time = Instant::now();

    // ファイルフィールドを取得
    let field = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::new(ApiErrorCode::InvalidInput, format!("マルチパートデータの解析に失敗: {}", e)))?
        .ok_or_else(|| ApiError::new(ApiErrorCode::InvalidInput, "ファイルフィールドが見つかりません"))?;

    let filename = field
        .file_name()
        .unwrap_or("audio")
        .to_string();

    let file_data = field
        .bytes()
        .await
        .map_err(|e| ApiError::new(ApiErrorCode::InvalidInput, format!("ファイルデータの読み込みに失敗: {}", e)))?;

    // 処理を実行
    let result = process_transcription(
        state.clone(),
        file_data.to_vec(),
        filename,
        TranscribeRequest {
            language: None,
            translate_to_english: Some(false),
            include_timestamps: Some(false),
        },
        start_time,
    ).await;

    // 統計情報を更新
    match &result {
        Ok(response) => {
            let mut stats = state.stats.lock().unwrap();
            stats.record_success(response.processing_time_ms);
        }
        Err(_) => {
            let mut stats = state.stats.lock().unwrap();
            stats.record_failure();
        }
    }

    result
}

/// タイムスタンプ付き文字起こしエンドポイント
pub async fn transcribe_with_timestamps(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> ApiResult<Json<TranscribeResponse>> {
    // 統計情報を更新
    {
        let mut stats = state.stats.lock().unwrap();
        stats.record_request();
    }

    let start_time = Instant::now();
    let mut request = TranscribeRequest {
        language: None,
        translate_to_english: Some(false),
        include_timestamps: Some(true),
    };

    let mut file_data = Vec::new();
    let mut filename = String::new();

    // マルチパートフィールドを処理
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::new(ApiErrorCode::InvalidInput, format!("マルチパートデータの解析に失敗: {}", e)))?
    {
        let field_name = field.name().unwrap_or("").to_string();

        match field_name.as_str() {
            "file" => {
                filename = field.file_name().unwrap_or("audio").to_string();
                file_data = field
                    .bytes()
                    .await
                    .map_err(|e| ApiError::new(ApiErrorCode::InvalidInput, format!("ファイルデータの読み込みに失敗: {}", e)))?
                    .to_vec();
            }
            "language" => {
                let language = field
                    .text()
                    .await
                    .map_err(|e| ApiError::new(ApiErrorCode::InvalidInput, format!("言語パラメータの読み込みに失敗: {}", e)))?;
                request.language = Some(language);
            }
            "translate_to_english" => {
                let translate = field
                    .text()
                    .await
                    .map_err(|e| ApiError::new(ApiErrorCode::InvalidInput, format!("翻訳パラメータの読み込みに失敗: {}", e)))?;
                request.translate_to_english = Some(translate.parse().unwrap_or(false));
            }
            _ => {} // 未知のフィールドは無視
        }
    }

    if file_data.is_empty() {
        return Err(ApiError::new(ApiErrorCode::InvalidInput, "ファイルが見つかりません"));
    }

    // 処理を実行
    let result = process_transcription(state.clone(), file_data, filename, request, start_time).await;

    // 統計情報を更新
    match &result {
        Ok(response) => {
            let mut stats = state.stats.lock().unwrap();
            stats.record_success(response.processing_time_ms);
        }
        Err(_) => {
            let mut stats = state.stats.lock().unwrap();
            stats.record_failure();
        }
    }

    result
}

/// 文字起こし処理の共通ロジック
async fn process_transcription(
    state: AppState,
    file_data: Vec<u8>,
    filename: String,
    request: TranscribeRequest,
    start_time: Instant,
) -> ApiResult<Json<TranscribeResponse>> {
    // ファイルサイズの検証
    let config = &state.config;
    let max_size = config.max_file_size_bytes();
    if file_data.len() > max_size {
        return Err(ApiError::new(
            ApiErrorCode::FileTooLarge,
            format!(
                "ファイルサイズが制限を超えています: {} > {}",
                format_file_size(file_data.len() as u64),
                format_file_size(max_size as u64)
            ),
        ));
    }

    // CPU集約的な処理をブロッキングスレッドで実行
    let config_clone = Arc::clone(&state.config);
    let whisper_engine = Arc::clone(&state.whisper_engine);

    let processing_result = tokio::task::spawn_blocking(move || {
        // 音声プロセッサを作成
        let mut audio_processor = AudioProcessor::new(&config_clone)?;

        // ファイル形式の検証
        if !audio_processor.is_supported_format(&filename) {
            return Err(anyhow::anyhow!(
                "サポートされていないファイル形式: {}",
                filename
            ));
        }

        // 音声データを処理
        let processed_audio = audio_processor.process_audio_from_bytes(&file_data, &filename)?;

        // 音声の長さを検証
        audio_processor.validate_audio_duration(&processed_audio.original_metadata)?;

        // 音声データの前処理
        let mut audio_samples = processed_audio.samples;
        preprocess_audio(&mut audio_samples);

        // Whisperエンジンを取得
        let engine_guard = whisper_engine.lock().unwrap();
        let engine = engine_guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("Whisperエンジンが初期化されていません"))?;

        // 文字起こし実行
        let language = request.language.as_deref();
        let translate_to_english = request.translate_to_english.unwrap_or(false);
        let include_timestamps = request.include_timestamps.unwrap_or(false);

        if include_timestamps {
            let result = engine.transcribe_with_timestamps(
                &audio_samples,
                translate_to_english,
                language,
            )?;

            Ok((
                result.text,
                Some(result.segments),
                result.language,
                processed_audio.duration_ms,
                result.processing_time_ms,
            ))
        } else {
            let text = engine.transcribe(&audio_samples)?;
            let processing_time = start_time.elapsed().as_millis() as u64;

            Ok((
                text,
                None,
                language.map(|s| s.to_string()),
                processed_audio.duration_ms,
                processing_time,
            ))
        }
    })
    .await
    .map_err(|e| ApiError::new(ApiErrorCode::InternalError, format!("処理スレッドエラー: {}", e)))?;

    match processing_result {
        Ok((text, segments, language, duration_ms, processing_time_ms)) => {
            Ok(Json(TranscribeResponse {
                text,
                language,
                duration_ms: Some(duration_ms),
                segments,
                processing_time_ms,
            }))
        }
        Err(e) => Err(ApiError::new(ApiErrorCode::ProcessingFailed, e.to_string())),
    }
}

/// 利用可能なモデル情報を取得
pub async fn get_models(State(state): State<AppState>) -> ApiResult<Json<ModelsResponse>> {
    let catalog = ModelCatalog::default();
    let models_dir = std::path::Path::new(&state.config.paths.models_dir);

    let mut models = Vec::new();

    for (_key, model_def) in &catalog.models {
        let file_path = models_dir.join(&model_def.file_name);
        let is_available = file_path.exists();

        let size_mb = if is_available {
            std::fs::metadata(&file_path)
                .map(|metadata| metadata.len() / (1024 * 1024))
                .unwrap_or(model_def.size_mb)
        } else {
            model_def.size_mb
        };

        models.push(ModelInfo {
            name: model_def.name.clone(),
            file_path: file_path.to_string_lossy().to_string(),
            size_mb,
            description: model_def.description.clone(),
            language_support: model_def.language_support.clone(),
            is_available,
        });
    }

    let current_model = state.config.whisper.default_model.clone();

    Ok(Json(ModelsResponse {
        models,
        current_model,
    }))
}

/// ヘルスチェックエンドポイント
pub async fn health_check(State(state): State<AppState>) -> Json<HealthResponse> {
    let uptime_seconds = state.start_time.elapsed().as_secs();

    let model_loaded = {
        let engine_guard = state.whisper_engine.lock().unwrap();
        engine_guard.is_some()
    };

    // メモリ使用量の取得（簡易版）
    let memory_usage_mb = get_memory_usage_mb();

    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        model_loaded,
        uptime_seconds,
        memory_usage_mb,
    })
}

/// サーバー統計情報を取得
pub async fn get_stats(State(state): State<AppState>) -> Json<ServerStats> {
    let mut stats = state.stats.lock().unwrap().clone();
    stats.uptime_seconds = state.start_time.elapsed().as_secs();
    Json(stats)
}

/// サポートされている言語のリストを取得
pub async fn get_languages() -> Json<Vec<LanguageInfo>> {
    let languages = get_supported_languages()
        .iter()
        .map(|&code| LanguageInfo {
            code: code.to_string(),
            name: get_language_name(code).to_string(),
        })
        .collect();

    Json(languages)
}

#[derive(serde::Serialize)]
pub struct LanguageInfo {
    pub code: String,
    pub name: String,
}

// =============================================================================
// Utility Functions
// =============================================================================

/// メモリ使用量を取得（簡易版）
fn get_memory_usage_mb() -> Option<u64> {
    // Linuxの場合は/proc/self/statusから取得
    #[cfg(target_os = "linux")]
    {
        if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
            for line in status.lines() {
                if line.starts_with("VmRSS:") {
                    if let Some(kb_str) = line.split_whitespace().nth(1) {
                        if let Ok(kb) = kb_str.parse::<u64>() {
                            return Some(kb / 1024); // KBからMBに変換
                        }
                    }
                }
            }
        }
    }

    // その他のプラットフォームでは未対応
    None
}

/// CORS対応のための追加ヘッダー
pub async fn add_cors_headers() -> impl axum::response::IntoResponse {
    (
        [
            ("Access-Control-Allow-Origin", "*"),
            ("Access-Control-Allow-Methods", "GET, POST, OPTIONS"),
            ("Access-Control-Allow-Headers", "Content-Type"),
        ],
        StatusCode::OK,
    )
}
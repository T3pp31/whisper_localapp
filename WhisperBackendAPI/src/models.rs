use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// API Request/Response Models
// - ハンドラとのデータ受け渡しに用いる型
// =============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct TranscribeRequest {
    /// 言語コード（例: "ja", "en", "auto"）。未指定の場合は設定に従う
    pub language: Option<String>,
    /// 英語へ翻訳するかどうか（true/false）
    pub translate_to_english: Option<bool>,
    /// セグメントのタイムスタンプを含めるかどうか
    pub include_timestamps: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TranscribeResponse {
    /// 文字起こしテキスト（全体）
    pub text: String,
    /// 推定/指定された言語コード
    pub language: Option<String>,
    /// 入力音声の長さ（ミリ秒）
    pub duration_ms: Option<u64>,
    /// セグメント情報（タイムスタンプ付き）
    pub segments: Option<Vec<TranscriptionSegment>>,
    /// サーバー側での処理時間（ミリ秒）
    pub processing_time_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelInfo {
    /// 論理名（UI 表示向け）
    pub name: String,
    /// 実ファイルパス（存在チェックに使用）
    pub file_path: String,
    /// ファイルサイズ（MB）
    pub size_mb: u64,
    /// モデルの説明
    pub description: String,
    /// 言語サポートの分類（例: multilingual）
    pub language_support: Vec<String>,
    /// サーバー上にダウンロード済みか
    pub is_available: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ModelsResponse {
    pub models: Vec<ModelInfo>,
    pub current_model: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: String,
    pub model_loaded: bool,
    pub uptime_seconds: u64,
    pub memory_usage_mb: Option<u64>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
    pub code: String,
    pub details: Option<String>,
}

// =============================================================================
// Core Data Models (from whisperGUIapp)
// - 文字起こし結果のセグメントなど、UI 側での再利用を想定
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptionSegment {
    pub text: String,
    pub start_time_ms: u64,
    pub end_time_ms: u64,
}

impl TranscriptionSegment {
    pub fn new(text: String, start_time_ms: u64, end_time_ms: u64) -> Self {
        Self {
            text,
            start_time_ms,
            end_time_ms,
        }
    }

    pub fn duration_ms(&self) -> u64 {
        self.end_time_ms.saturating_sub(self.start_time_ms)
    }

    pub fn to_srt_format(&self, index: usize) -> String {
        let start_time = Self::ms_to_srt_time(self.start_time_ms);
        let end_time = Self::ms_to_srt_time(self.end_time_ms);

        format!(
            "{}\n{} --> {}\n{}\n\n",
            index + 1,
            start_time,
            end_time,
            self.text
        )
    }

    pub fn to_vtt_format(&self) -> String {
        let start_time = Self::ms_to_vtt_time(self.start_time_ms);
        let end_time = Self::ms_to_vtt_time(self.end_time_ms);

        format!("{} --> {}\n{}\n\n", start_time, end_time, self.text)
    }

    fn ms_to_srt_time(ms: u64) -> String {
        // 例: 00:01:23,456 の形式に変換
        let total_seconds = ms / 1000;
        let milliseconds = ms % 1000;
        let seconds = total_seconds % 60;
        let minutes = (total_seconds / 60) % 60;
        let hours = total_seconds / 3600;

        format!(
            "{:02}:{:02}:{:02},{:03}",
            hours, minutes, seconds, milliseconds
        )
    }

    fn ms_to_vtt_time(ms: u64) -> String {
        // 例: 00:01:23.456 の形式に変換
        let total_seconds = ms / 1000;
        let milliseconds = ms % 1000;
        let seconds = total_seconds % 60;
        let minutes = (total_seconds / 60) % 60;
        let hours = total_seconds / 3600;

        format!(
            "{:02}:{:02}:{:02}.{:03}",
            hours, minutes, seconds, milliseconds
        )
    }
}

// =============================================================================
// Model Catalog
// - UI のモデル選択やダウンロードリンク表示に使うメタ情報
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelCatalog {
    pub models: HashMap<String, ModelDefinition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDefinition {
    pub name: String,
    pub file_name: String,
    pub download_url: String,
    pub size_mb: u64,
    pub description: String,
    pub language_support: Vec<String>,
    pub quality: ModelQuality,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ModelQuality {
    Tiny,
    Base,
    Small,
    Medium,
    Large,
    LargeV2,
    LargeV3,
}

impl ModelQuality {
    pub fn to_string(&self) -> &'static str {
        match self {
            ModelQuality::Tiny => "tiny",
            ModelQuality::Base => "base",
            ModelQuality::Small => "small",
            ModelQuality::Medium => "medium",
            ModelQuality::Large => "large",
            ModelQuality::LargeV2 => "large-v2",
            ModelQuality::LargeV3 => "large-v3",
        }
    }
}

impl Default for ModelCatalog {
    fn default() -> Self {
        let mut models = HashMap::new();

        models.insert(
            "tiny".to_string(),
            ModelDefinition {
                name: "Whisper Tiny".to_string(),
                file_name: "ggml-tiny.bin".to_string(),
                download_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin".to_string(),
                size_mb: 39,
                description: "最小モデル（39MB）- 高速だが精度は低い".to_string(),
                language_support: vec!["multilingual".to_string()],
                quality: ModelQuality::Tiny,
            },
        );

        models.insert(
            "base".to_string(),
            ModelDefinition {
                name: "Whisper Base".to_string(),
                file_name: "ggml-base.bin".to_string(),
                download_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin".to_string(),
                size_mb: 142,
                description: "基本モデル（142MB）- バランスの取れた速度と精度".to_string(),
                language_support: vec!["multilingual".to_string()],
                quality: ModelQuality::Base,
            },
        );

        models.insert(
            "small".to_string(),
            ModelDefinition {
                name: "Whisper Small".to_string(),
                file_name: "ggml-small.bin".to_string(),
                download_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin".to_string(),
                size_mb: 244,
                description: "小型モデル（244MB）- 良好な精度と実用的な速度".to_string(),
                language_support: vec!["multilingual".to_string()],
                quality: ModelQuality::Small,
            },
        );

        models.insert(
            "medium".to_string(),
            ModelDefinition {
                name: "Whisper Medium".to_string(),
                file_name: "ggml-medium.bin".to_string(),
                download_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin".to_string(),
                size_mb: 769,
                description: "中型モデル（769MB）- 高精度だが処理時間が長い".to_string(),
                language_support: vec!["multilingual".to_string()],
                quality: ModelQuality::Medium,
            },
        );

        models.insert(
            "large-v3-turbo-q5_0".to_string(),
            ModelDefinition {
                name: "Whisper Large V3 Turbo Q5_0".to_string(),
                file_name: "ggml-large-v3-turbo-q5_0.bin".to_string(),
                download_url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin".to_string(),
                size_mb: 809,
                description: "最新の大型モデル（809MB）- 最高精度、量子化により高速化".to_string(),
                language_support: vec!["multilingual".to_string()],
                quality: ModelQuality::LargeV3,
            },
        );

        Self { models }
    }
}

// =============================================================================
// Server State and Statistics
// - API 利用状況を簡易的に記録（監視/デバッグ用）
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerStats {
    pub total_requests: u64,
    pub successful_transcriptions: u64,
    pub failed_transcriptions: u64,
    pub total_processing_time_ms: u64,
    pub total_audio_duration_ms: u64,
    pub average_processing_time_ms: f64,
    pub active_requests: usize,
    pub uptime_seconds: u64,
}

impl Default for ServerStats {
    fn default() -> Self {
        Self {
            total_requests: 0,
            successful_transcriptions: 0,
            failed_transcriptions: 0,
            total_processing_time_ms: 0,
            total_audio_duration_ms: 0,
            average_processing_time_ms: 0.0,
            active_requests: 0,
            uptime_seconds: 0,
        }
    }
}

impl ServerStats {
    pub fn record_request(&mut self) {
        self.total_requests += 1;
        self.active_requests += 1;
    }

    pub fn record_success(&mut self, processing_time_ms: u64, audio_duration_ms: Option<u64>) {
        self.successful_transcriptions += 1;
        self.active_requests = self.active_requests.saturating_sub(1);
        self.total_processing_time_ms = self.total_processing_time_ms.saturating_add(processing_time_ms);

        if let Some(duration_ms) = audio_duration_ms {
            self.total_audio_duration_ms = self.total_audio_duration_ms.saturating_add(duration_ms);
        }

        if self.total_audio_duration_ms > 0 {
            let processing_per_ms = self.total_processing_time_ms as f64 / self.total_audio_duration_ms as f64;
            self.average_processing_time_ms = processing_per_ms * 60_000.0;
        } else {
            self.average_processing_time_ms = 0.0;
        }
    }

    pub fn record_failure(&mut self) {
        self.failed_transcriptions += 1;
        self.active_requests = self.active_requests.saturating_sub(1);
    }

    pub fn success_rate(&self) -> f64 {
        if self.total_requests == 0 {
            0.0
        } else {
            self.successful_transcriptions as f64 / self.total_requests as f64 * 100.0
        }
    }
}

// =============================================================================
// Error Types
// - ハンドラ→レスポンス変換で HTTP ステータスにマッピング
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiErrorCode {
    InvalidInput,
    FileTooLarge,
    UnsupportedFormat,
    ProcessingFailed,
    ModelNotLoaded,
    ServerOverloaded,
    InternalError,
}

impl ApiErrorCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            ApiErrorCode::InvalidInput => "INVALID_INPUT",
            ApiErrorCode::FileTooLarge => "FILE_TOO_LARGE",
            ApiErrorCode::UnsupportedFormat => "UNSUPPORTED_FORMAT",
            ApiErrorCode::ProcessingFailed => "PROCESSING_FAILED",
            ApiErrorCode::ModelNotLoaded => "MODEL_NOT_LOADED",
            ApiErrorCode::ServerOverloaded => "SERVER_OVERLOADED",
            ApiErrorCode::InternalError => "INTERNAL_ERROR",
        }
    }
}

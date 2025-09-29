// WhisperBackendAPI ライブラリ
// テストから各モジュールにアクセスできるようにするため

pub mod audio;
pub mod config;
pub mod models;

// whisper関連のモジュールは条件コンパイル
#[cfg(feature = "whisper")]
pub mod whisper;

#[cfg(not(feature = "whisper"))]
pub mod whisper {
    // whisper機能が無効の場合のモック実装
    use crate::config::Config;
    use anyhow::Result;

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
    pub struct ModelInfo {
        pub is_loaded: bool,
        pub language: Option<String>,
        pub threads: i32,
        pub enable_gpu: bool,
    }

    #[derive(Debug, Clone)]
    pub struct TranscriptionResult {
        pub text: String,
        pub segments: Vec<crate::models::TranscriptionSegment>,
        pub language: Option<String>,
        pub processing_time_ms: u64,
    }

    pub struct WhisperEngine;

    impl WhisperEngine {
        pub fn new(_model_path: &str, _config: &Config) -> Result<Self> {
            Err(anyhow::anyhow!(
                "Whisper engine not available (feature disabled)"
            ))
        }

        pub fn get_model_info(&self) -> ModelInfo {
            ModelInfo {
                is_loaded: false,
                language: None,
                threads: 1,
                enable_gpu: false,
            }
        }
    }

    /// サポートされている言語のリストを取得
    pub fn get_supported_languages() -> Vec<&'static str> {
        vec![
            "auto", "en", "zh", "de", "es", "ru", "ko", "fr", "ja", "pt", "tr", "pl", "ca", "nl",
            "ar", "sv", "it", "id", "hi", "fi", "vi", "he", "uk", "el", "ms", "cs", "ro", "da",
            "hu", "ta", "no", "th", "ur", "hr", "bg", "lt", "la", "mi", "ml", "cy", "sk", "te",
            "fa", "lv", "bn", "sr", "az", "sl", "kn", "et", "mk", "br", "eu", "is", "hy", "ne",
            "mn", "bs", "kk", "sq", "sw", "gl", "mr", "pa", "si", "km", "sn", "yo", "so", "af",
            "oc", "ka", "be", "tg", "sd", "gu", "am", "yi", "lo", "uz", "fo", "ht", "ps", "tk",
            "nn", "mt", "sa", "lb", "my", "bo", "tl", "mg", "as", "tt", "haw", "ln", "ha", "ba",
            "jw", "su",
        ]
    }

    /// 言語コードから言語名を取得
    pub fn get_language_name(code: &str) -> &'static str {
        match code {
            "en" => "English",
            "zh" => "Chinese",
            "de" => "German",
            "es" => "Spanish",
            "ru" => "Russian",
            "ko" => "Korean",
            "fr" => "French",
            "ja" => "Japanese",
            "pt" => "Portuguese",
            "tr" => "Turkish",
            "pl" => "Polish",
            "ca" => "Catalan",
            "nl" => "Dutch",
            "ar" => "Arabic",
            "sv" => "Swedish",
            "it" => "Italian",
            "auto" => "Auto Detect",
            _ => "Unknown",
        }
    }

    /// 音声データの前処理（ノイズ除去等）
    pub fn preprocess_audio(audio_data: &mut [f32]) {
        normalize_audio(audio_data);
    }

    /// 音声データの正規化
    fn normalize_audio(audio_data: &mut [f32]) {
        if audio_data.is_empty() {
            return;
        }

        // 最大絶対値を見つける
        let max_abs = audio_data.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);

        if max_abs > 0.0 {
            // 正規化係数を計算（最大値を0.95に制限）
            let normalize_factor = 0.95 / max_abs;

            // 正規化を適用
            for sample in audio_data.iter_mut() {
                *sample *= normalize_factor;
            }
        }
    }
}

// handlersモジュールも条件コンパイル
#[cfg(feature = "whisper")]
pub mod handlers;

#[cfg(not(feature = "whisper"))]
pub mod handlers {
    // whisper機能が無効の場合のモック実装
    use crate::config::Config;
    use crate::models::ServerStats;
    use std::sync::{Arc, Mutex};
    use std::time::Instant;

    #[derive(Clone)]
    pub struct AppState {
        pub config: Arc<Config>,
        pub whisper_engine: Arc<Mutex<Option<crate::whisper::WhisperEngine>>>,
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
    }

    #[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
}

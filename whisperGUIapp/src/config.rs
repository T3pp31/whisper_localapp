//! アプリ全体の設定（モデル/音声/GUI/性能/入出力/各種パス）を管理するモジュール。
//! - `Config::load()` でユーザー領域の設定を読み込み（なければデフォルト生成）
//! - `Config::save()` で保存先に書き出し
//! - `ensure_directories()` で必要なディレクトリを作成

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// アプリ全体の設定ルート。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub whisper: WhisperConfig,
    pub audio: AudioConfig,
    pub gui: GuiConfig,
    pub performance: PerformanceConfig,
    pub paths: PathsConfig,
    pub output: OutputConfig,
}

/// Whisper に関する設定。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhisperConfig {
    pub model_path: String,
    pub default_model: String,
    pub language: String,
    #[serde(default)]
    pub use_remote_server: bool,
    #[serde(default = "default_remote_server_url")]
    pub remote_server_url: String,
    #[serde(default = "default_remote_server_endpoint")]
    pub remote_server_endpoint: String,
}

/// 音声処理（前処理）に関する設定。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub channels: u32,
    pub buffer_size: usize,
}

/// GUI 表示に関する設定。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuiConfig {
    pub window_width: f32,
    pub window_height: f32,
    pub window_title: String,
    pub theme: String,
}

/// スレッド数や GPU 使用の有無など性能関連の設定。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    pub audio_threads: usize,
    pub whisper_threads: usize,
    pub use_gpu: bool,
}

/// モデル/出力/一時ファイルのディレクトリ。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathsConfig {
    pub models_dir: String,
    pub output_dir: String,
    pub temp_dir: String,
}

/// 出力フォーマット等の設定。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    pub default_format: String,
    pub supported_formats: Vec<String>,
    pub auto_save: bool,
}

impl Config {
    /// 設定をユーザー領域から読み込む。存在しなければデフォルトを作成して保存。
    pub fn load() -> Result<Self> {
        let path = Self::config_file_path();

        if path.exists() {
            let content = fs::read_to_string(&path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else if Path::new("config.toml").exists() {
            // 旧バージョン互換: カレント直下の設定があれば読み込み
            let content = fs::read_to_string("config.toml")?;
            let config: Config = toml::from_str(&content)?;
            // 新しい保存先に移行
            config.save()?;
            Ok(config)
        } else {
            // デフォルト設定を作成し、新しい保存先に書き出す
            let default_config = Self::default();
            default_config.save()?;
            Ok(default_config)
        }
    }

    /// 現在の設定をユーザー領域に保存する。
    pub fn save(&self) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        let path = Self::config_file_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, content)?;
        Ok(())
    }

    /// 必要なディレクトリ（models/output/temp）を作成する。
    pub fn ensure_directories(&self) -> Result<()> {
        fs::create_dir_all(&self.paths.models_dir)?;
        fs::create_dir_all(&self.paths.output_dir)?;
        fs::create_dir_all(&self.paths.temp_dir)?;
        Ok(())
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            whisper: WhisperConfig {
                model_path: "models/ggml-base.bin".to_string(),
                default_model: "base".to_string(),
                language: "ja".to_string(),
                use_remote_server: false,
                remote_server_url: default_remote_server_url(),
                remote_server_endpoint: default_remote_server_endpoint(),
            },
            audio: AudioConfig {
                sample_rate: 16000,
                channels: 1,
                buffer_size: 4096,
            },
            gui: GuiConfig {
                window_width: 800.0,
                window_height: 600.0,
                window_title: "Whisper音声文字起こし".to_string(),
                theme: "Light".to_string(),
            },
            performance: PerformanceConfig {
                audio_threads: 2,
                whisper_threads: 4,
                use_gpu: false,
            },
            paths: PathsConfig {
                models_dir: "models".to_string(),
                output_dir: "output".to_string(),
                temp_dir: "temp".to_string(),
            },
            output: OutputConfig {
                default_format: "txt".to_string(),
                supported_formats: vec!["txt".to_string(), "srt".to_string(), "vtt".to_string()],
                auto_save: true,
            },
        }
    }
}

impl Config {
    /// アプリの設定保存先（ユーザー領域）。
    /// 例: `<config_dir>/whisperGUIapp/config.toml`
    fn config_file_path() -> PathBuf {
        let base = dirs_next::config_dir().unwrap_or_else(|| PathBuf::from("."));
        base.join("whisperGUIapp").join("config.toml")
    }
}

fn default_remote_server_url() -> String {
    "http://127.0.0.1:8080".to_string()
}

fn default_remote_server_endpoint() -> String {
    "/transcribe-with-timestamps".to_string()
}

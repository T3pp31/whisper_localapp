use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub whisper: WhisperConfig,
    pub audio: AudioConfig,
    pub gui: GuiConfig,
    pub performance: PerformanceConfig,
    pub paths: PathsConfig,
    pub output: OutputConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhisperConfig {
    pub model_path: String,
    pub default_model: String,
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    pub sample_rate: u32,
    pub channels: u32,
    pub buffer_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuiConfig {
    pub window_width: f32,
    pub window_height: f32,
    pub window_title: String,
    pub theme: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    pub audio_threads: usize,
    pub whisper_threads: usize,
    pub use_gpu: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathsConfig {
    pub models_dir: String,
    pub output_dir: String,
    pub temp_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputConfig {
    pub default_format: String,
    pub supported_formats: Vec<String>,
    pub auto_save: bool,
}

impl Config {
    pub fn load() -> Result<Self> {
        let config_path = "config.toml";

        if Path::new(config_path).exists() {
            let content = fs::read_to_string(config_path)?;
            let config: Config = toml::from_str(&content)?;
            Ok(config)
        } else {
            // デフォルト設定を作成
            let default_config = Self::default();
            default_config.save()?;
            Ok(default_config)
        }
    }

    pub fn save(&self) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        fs::write("config.toml", content)?;
        Ok(())
    }

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

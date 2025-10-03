//! 設定モジュール（YAML 読み込み）
//!
//! `ConfigSet` はルートディレクトリ配下の複数YAMLファイルを読み込み、
//! 実行時に必要な設定値を型安全に提供します。
mod asr;
mod audio;
mod error;
mod monitoring;
mod server;
mod whisper;
mod system;

use std::fs;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;

pub use asr::*;
pub use audio::*;
pub use error::ConfigError;
pub use monitoring::*;
pub use server::*;
pub use whisper::*;
pub use system::*;

/// 設定ディレクトリを指す環境変数名
pub const CONFIG_DIR_ENV: &str = "WHISPER_REALTIME_CONFIG_DIR";

/// すべての設定をひとまとめにした構造体
#[derive(Debug, Clone)]
pub struct ConfigSet {
    pub system: SystemRequirements,
    pub audio: AudioProcessingConfig,
    pub asr: AsrPipelineConfig,
    pub monitoring: MonitoringConfig,
    pub server: ServerConfig,
    pub whisper: WhisperModelConfig,
    root: PathBuf,
}

impl ConfigSet {
    /// ルートディレクトリから各YAMLを読み込み
    pub fn load_from_dir<P: AsRef<Path>>(dir: P) -> Result<Self, ConfigError> {
        let root = dir.as_ref().to_path_buf();
        if !root.is_dir() {
            return Err(ConfigError::MissingRoot(root));
        }

        let system = load_yaml(root.join("system_requirements.yaml"))?;
        let audio = load_yaml(root.join("audio_processing.yaml"))?;
        let asr = load_yaml(root.join("asr_pipeline.yaml"))?;
        let monitoring = load_yaml(root.join("monitoring.yaml"))?;
        let server = load_yaml(root.join("server.yaml"))?;
        let whisper = load_yaml(root.join("whisper_model.yaml"))?;

        Ok(Self {
            system,
            audio,
            asr,
            monitoring,
            server,
            whisper,
            root,
        })
    }

    /// 環境変数（未設定時は `config/`）から設定を読み込み
    pub fn load_from_env() -> Result<Self, ConfigError> {
        let dir = std::env::var(CONFIG_DIR_ENV).unwrap_or_else(|_| "config".to_string());
        Self::load_from_dir(dir)
    }

    /// 設定ルートのパス（デバッグ等に利用）
    pub fn root(&self) -> &Path {
        &self.root
    }
}

/// YAMLファイルを読み込み、型 `T` へデシリアライズ
fn load_yaml<T>(path: PathBuf) -> Result<T, ConfigError>
where
    T: DeserializeOwned,
{
    let data = fs::read_to_string(&path).map_err(|source| ConfigError::Io {
        path: path.clone(),
        source,
    })?;
    serde_yaml::from_str(&data).map_err(|source| ConfigError::Parse { path, source })
}

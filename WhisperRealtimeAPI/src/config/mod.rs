mod asr;
mod audio;
mod error;
mod monitoring;
mod system;

use std::fs;
use std::path::{Path, PathBuf};

use serde::de::DeserializeOwned;

pub use asr::*;
pub use audio::*;
pub use error::ConfigError;
pub use monitoring::*;
pub use system::*;

pub const CONFIG_DIR_ENV: &str = "WHISPER_REALTIME_CONFIG_DIR";

#[derive(Debug, Clone)]
pub struct ConfigSet {
    pub system: SystemRequirements,
    pub audio: AudioProcessingConfig,
    pub asr: AsrPipelineConfig,
    pub monitoring: MonitoringConfig,
    root: PathBuf,
}

impl ConfigSet {
    pub fn load_from_dir<P: AsRef<Path>>(dir: P) -> Result<Self, ConfigError> {
        let root = dir.as_ref().to_path_buf();
        if !root.is_dir() {
            return Err(ConfigError::MissingRoot(root));
        }

        let system = load_yaml(root.join("system_requirements.yaml"))?;
        let audio = load_yaml(root.join("audio_processing.yaml"))?;
        let asr = load_yaml(root.join("asr_pipeline.yaml"))?;
        let monitoring = load_yaml(root.join("monitoring.yaml"))?;

        Ok(Self {
            system,
            audio,
            asr,
            monitoring,
            root,
        })
    }

    pub fn load_from_env() -> Result<Self, ConfigError> {
        let dir = std::env::var(CONFIG_DIR_ENV).unwrap_or_else(|_| "config".to_string());
        Self::load_from_dir(dir)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }
}

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

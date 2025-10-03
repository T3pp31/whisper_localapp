//! 設定読み込み時のエラー定義
use std::path::PathBuf;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("missing configuration directory: {0:?}")]
    MissingRoot(PathBuf),
    #[error("failed to read configuration file: {path:?}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("failed to parse configuration file: {path:?}")]
    Parse {
        path: PathBuf,
        #[source]
        source: serde_yaml::Error,
    },
}

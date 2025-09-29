use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub backend: BackendConfig,
    pub webui: WebUIConfig,
    #[serde(default)]
    pub realtime: RealtimeConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    #[serde(default = "ServerConfig::default_max_request_size_mb")]
    pub max_request_size_mb: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackendConfig {
    pub base_url: String,
    pub timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RealtimeConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub config_dir: Option<String>,
    #[serde(default)]
    pub default_client_type: Option<String>,
    #[serde(default)]
    pub default_client_name: Option<String>,
    #[serde(default)]
    pub default_client_version: Option<String>,
    #[serde(default)]
    pub default_token_subject: Option<String>,
    #[serde(default = "RealtimeConfig::default_heartbeat_interval_ms")]
    pub heartbeat_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebUIConfig {
    pub title: String,
    pub max_file_size_mb: u64,
    pub allowed_extensions: Vec<String>,
    #[serde(default)]
    pub default_language: Option<String>,
    #[serde(default = "WebUIConfig::default_timeline_update_interval_ms")]
    pub timeline_update_interval_ms: u64,
    #[serde(default = "WebUIConfig::default_upload_prompt_text")]
    pub upload_prompt_text: String,
    #[serde(default = "WebUIConfig::default_upload_success_text")]
    pub upload_success_text: String,
    #[serde(default = "WebUIConfig::default_stats_average_processing_time_label")]
    pub stats_average_processing_time_label: String,
    #[serde(default = "WebUIConfig::default_stats_average_processing_time_unit")]
    pub stats_average_processing_time_unit: String,
}

impl ServerConfig {
    const fn default_max_request_size_mb() -> u64 {
        110
    }
}

impl RealtimeConfig {
    const fn default_heartbeat_interval_ms() -> u64 {
        30_000
    }

    pub fn config_dir_path(&self) -> Option<PathBuf> {
        self.config_dir
            .as_ref()
            .map(|value| Path::new(value).to_path_buf())
    }
}

impl Default for RealtimeConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            config_dir: None,
            default_client_type: None,
            default_client_name: None,
            default_client_version: None,
            default_token_subject: None,
            heartbeat_interval_ms: Self::default_heartbeat_interval_ms(),
        }
    }
}

impl WebUIConfig {
    const fn default_timeline_update_interval_ms() -> u64 {
        200
    }

    fn default_upload_prompt_text() -> String {
        "音声ファイルをドラッグ&ドロップするか、クリックして選択してください".to_string()
    }

    fn default_upload_success_text() -> String {
        "{filename} を選択しました".to_string()
    }

    fn default_stats_average_processing_time_label() -> String {
        "平均処理時間 (音声1分あたりの文字起こし所要時間)".to_string()
    }

    fn default_stats_average_processing_time_unit() -> String {
        "秒 / 音声1分".to_string()
    }
}

impl Config {
    pub fn load_or_create_default<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let path = path.as_ref();

        if path.exists() {
            let content = fs::read_to_string(path)?;
            let config: Config = toml::from_str(&content)?;
            config.validate()?;
            Ok(config)
        } else {
            let default_config = Self::default();
            let content = toml::to_string(&default_config)?;
            fs::write(path, content)?;
            println!("デフォルト設定ファイルを作成しました: {}", path.display());
            Ok(default_config)
        }
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        if self.server.port == 0 {
            return Err(anyhow::anyhow!("サーバーポートが無効です"));
        }

        if self.server.max_request_size_mb == 0 {
            return Err(anyhow::anyhow!("最大リクエストサイズが無効です"));
        }

        if self.server.max_request_size_mb < self.webui.max_file_size_mb {
            return Err(anyhow::anyhow!(
                "最大リクエストサイズは最大ファイルサイズ以上である必要があります"
            ));
        }

        if self.backend.base_url.is_empty() {
            return Err(anyhow::anyhow!("バックエンドURLが設定されていません"));
        }

        if self.webui.max_file_size_mb == 0 {
            return Err(anyhow::anyhow!("最大ファイルサイズが無効です"));
        }

        if self.webui.timeline_update_interval_ms == 0 {
            return Err(anyhow::anyhow!("タイムライン更新間隔が無効です"));
        }

        if self.webui.upload_prompt_text.trim().is_empty() {
            return Err(anyhow::anyhow!(
                "アップロード案内テキストが設定されていません"
            ));
        }

        if self.webui.upload_success_text.trim().is_empty() {
            return Err(anyhow::anyhow!(
                "アップロード完了テキストが設定されていません"
            ));
        }

        if self
            .webui
            .stats_average_processing_time_label
            .trim()
            .is_empty()
        {
            return Err(anyhow::anyhow!(
                "平均処理時間表示ラベルが設定されていません"
            ));
        }

        if self.realtime.enabled {
            if self.realtime.heartbeat_interval_ms == 0 {
                return Err(anyhow::anyhow!(
                    "リアルタイム設定のハートビート間隔が無効です"
                ));
            }

            if self
                .realtime
                .config_dir
                .as_ref()
                .map(|dir| dir.trim().is_empty())
                .unwrap_or(true)
            {
                return Err(anyhow::anyhow!(
                    "リアルタイム設定のconfig_dirが指定されていません"
                ));
            }

            Self::validate_realtime_field(
                &self.realtime.default_client_type,
                "リアルタイム設定のデフォルトクライアント種別",
            )?;
            Self::validate_realtime_field(
                &self.realtime.default_client_name,
                "リアルタイム設定のデフォルトクライアント名",
            )?;
            Self::validate_realtime_field(
                &self.realtime.default_client_version,
                "リアルタイム設定のデフォルトクライアントバージョン",
            )?;
            Self::validate_realtime_field(
                &self.realtime.default_token_subject,
                "リアルタイム設定のデフォルトトークンサブジェクト",
            )?;
        }

        Ok(())
    }

    fn validate_realtime_field(value: &Option<String>, label: &str) -> anyhow::Result<()> {
        match value {
            Some(field) if !field.trim().is_empty() => Ok(()),
            _ => Err(anyhow::anyhow!("{}が設定されていません", label)),
        }
    }

    pub fn server_address(&self) -> String {
        format!("{}:{}", self.server.host, self.server.port)
    }

    pub fn max_file_size_bytes(&self) -> usize {
        (self.webui.max_file_size_mb * 1024 * 1024) as usize
    }

    pub fn max_request_size_bytes(&self) -> usize {
        (self.server.max_request_size_mb * 1024 * 1024) as usize
    }

    pub fn is_allowed_extension(&self, extension: &str) -> bool {
        self.webui
            .allowed_extensions
            .iter()
            .any(|ext| ext.eq_ignore_ascii_case(extension))
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "127.0.0.1".to_string(),
                port: 3000,
                max_request_size_mb: ServerConfig::default_max_request_size_mb(),
            },
            backend: BackendConfig {
                base_url: "http://127.0.0.1:8000".to_string(),
                timeout_seconds: 300,
            },
            webui: WebUIConfig {
                title: "Whisper WebUI".to_string(),
                max_file_size_mb: 100,
                allowed_extensions: vec![
                    "wav".to_string(),
                    "mp3".to_string(),
                    "m4a".to_string(),
                    "flac".to_string(),
                    "ogg".to_string(),
                    "mp4".to_string(),
                    "mov".to_string(),
                    "avi".to_string(),
                    "mkv".to_string(),
                ],
                default_language: None,
                timeline_update_interval_ms: WebUIConfig::default_timeline_update_interval_ms(),
                upload_prompt_text: WebUIConfig::default_upload_prompt_text(),
                upload_success_text: WebUIConfig::default_upload_success_text(),
                stats_average_processing_time_label:
                    WebUIConfig::default_stats_average_processing_time_label(),
                stats_average_processing_time_unit:
                    WebUIConfig::default_stats_average_processing_time_unit(),
            },
            realtime: RealtimeConfig::default(),
        }
    }
}

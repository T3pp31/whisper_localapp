use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

// =============================================================================
// 設定モデル
// - サーバー/Whisper/音声処理/性能/パス/制限の各カテゴリで構成
// - `Config::load_or_create_default` で既定ファイル生成にも対応
// =============================================================================
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub server: ServerConfig,
    pub whisper: WhisperConfig,
    pub audio: AudioConfig,
    pub performance: PerformanceConfig,
    pub paths: PathsConfig,
    pub limits: LimitsConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// バインドするホスト（例: 0.0.0.0）
    pub host: String,
    /// バインドするポート（例: 8080）
    pub port: u16,
    /// 許可する CORS オリジン
    pub cors_origins: Vec<String>,
    /// リクエストの最大サイズ（バイト）
    pub max_request_size: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhisperConfig {
    /// Whisper モデルの実ファイルパス
    pub model_path: String,
    /// UI/情報表示用の既定モデル名
    pub default_model: String,
    /// 既定の言語設定（"auto" は自動検出）
    pub language: String,
    /// GPU を有効にするか（whisper-rs の対応に依存）
    pub enable_gpu: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AudioConfig {
    /// ターゲットサンプリングレート（Hz）
    pub sample_rate: u32,
    /// チャンネル数（現状モノラル前提）
    pub channels: u16,
    /// デコード時に利用するバッファサイズ
    pub buffer_size: usize,
    /// 許可される拡張子（簡易判定）
    pub supported_formats: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerformanceConfig {
    /// 音声入出力処理に割くスレッド数（未使用の場合あり）
    pub audio_threads: usize,
    /// Whisper 推論に割くスレッド数
    pub whisper_threads: usize,
    /// API の同時処理上限（現状は統計のみ）
    pub max_concurrent_requests: usize,
    /// リクエストのタイムアウト（秒）
    pub request_timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathsConfig {
    /// モデル配置ディレクトリ
    pub models_dir: String,
    /// 一時ファイルディレクトリ
    pub temp_dir: String,
    /// アップロード保存ディレクトリ
    pub upload_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitsConfig {
    /// アップロード最大ファイルサイズ（MB）
    pub max_file_size_mb: usize,
    /// 音声の最大長（分）
    pub max_audio_duration_minutes: u32,
    /// 一時ファイルの自動クリーンアップまでの時間（分）
    pub cleanup_temp_files_after_minutes: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            server: ServerConfig {
                host: "0.0.0.0".to_string(),
                port: 8080,
                cors_origins: vec!["*".to_string()],
                max_request_size: 100 * 1024 * 1024, // 100MB
            },
            whisper: WhisperConfig {
                model_path: "models/ggml-large-v3-turbo-q5_0.bin".to_string(),
                default_model: "large-q5_0".to_string(),
                language: "auto".to_string(),
                enable_gpu: true,
            },
            audio: AudioConfig {
                sample_rate: 16000,
                channels: 1,
                buffer_size: 4096,
                supported_formats: vec![
                    "wav".to_string(),
                    "mp3".to_string(),
                    "mp4".to_string(),
                    "m4a".to_string(),
                    "flac".to_string(),
                    "ogg".to_string(),
                ],
            },
            performance: PerformanceConfig {
                audio_threads: 10,
                whisper_threads: 14,
                max_concurrent_requests: 10,
                request_timeout_seconds: 300, // 5 minutes
            },
            paths: PathsConfig {
                models_dir: "models".to_string(),
                temp_dir: "temp".to_string(),
                upload_dir: "uploads".to_string(),
            },
            limits: LimitsConfig {
                max_file_size_mb: 50,
                max_audio_duration_minutes: 180,
                cleanup_temp_files_after_minutes: 60,
            },
        }
    }
}

impl Config {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn save_to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let content = toml::to_string_pretty(self)?;
        fs::write(path, content)?;
        Ok(())
    }

    pub fn load_or_create_default<P: AsRef<Path>>(path: P) -> Result<Self> {
        if path.as_ref().exists() {
            match Self::load_from_file(&path) {
                Ok(config) => Ok(config),
                Err(e) => {
                    eprintln!(
                        "設定ファイルの読み込みに失敗しました: {}. デフォルト設定を使用します。",
                        e
                    );
                    let config = Self::default();
                    config.save_to_file(&path)?;
                    Ok(config)
                }
            }
        } else {
            let config = Self::default();
            config.save_to_file(&path)?;
            println!(
                "デフォルト設定ファイルを作成しました: {}",
                path.as_ref().display()
            );
            Ok(config)
        }
    }

    pub fn validate(&self) -> Result<()> {
        // ポート番号の検証
        if self.server.port == 0 {
            return Err(anyhow::anyhow!("無効なポート番号: {}", self.server.port));
        }

        // モデルファイルの存在確認
        // - 初回起動時など未ダウンロードの可能性あり
        if !Path::new(&self.whisper.model_path).exists() {
            return Err(anyhow::anyhow!(
                "Whisperモデルファイルが見つかりません: {}\n\
                 以下のコマンドでモデルをダウンロードしてください:\n\
                 wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin -P models/",
                self.whisper.model_path
            ));
        }

        // ディレクトリの存在確認と作成
        // - models/temp/uploads が無い場合は作成
        for dir in &[
            &self.paths.models_dir,
            &self.paths.temp_dir,
            &self.paths.upload_dir,
        ] {
            if !Path::new(dir).exists() {
                fs::create_dir_all(dir)
                    .map_err(|e| anyhow::anyhow!("ディレクトリの作成に失敗: {} - {}", dir, e))?;
            }
        }

        // パフォーマンス設定の検証
        if self.performance.whisper_threads == 0 {
            return Err(anyhow::anyhow!(
                "Whisperスレッド数は1以上である必要があります"
            ));
        }

        if self.performance.max_concurrent_requests == 0 {
            return Err(anyhow::anyhow!(
                "最大同時リクエスト数は1以上である必要があります"
            ));
        }

        // ファイルサイズ制限の検証
        if self.limits.max_file_size_mb == 0 {
            return Err(anyhow::anyhow!(
                "最大ファイルサイズは1MB以上である必要があります"
            ));
        }

        Ok(())
    }

    pub fn server_address(&self) -> String {
        format!("{}:{}", self.server.host, self.server.port)
    }

    pub fn max_file_size_bytes(&self) -> usize {
        self.limits.max_file_size_mb * 1024 * 1024
    }
}

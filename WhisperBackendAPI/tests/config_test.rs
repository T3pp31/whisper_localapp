use std::fs;
use std::path::Path;
use tempfile::TempDir;
use WhisperBackendAPI::config::*;

#[cfg(test)]
mod config_tests {
    use super::*;

    /// Configのデフォルト値テスト
    #[test]
    fn test_config_default() {
        let config = Config::default();

        // サーバー設定
        assert_eq!(config.server.host, "0.0.0.0");
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.server.cors_origins, vec!["*"]);
        assert_eq!(config.server.max_request_size, 100 * 1024 * 1024);

        // Whisper設定
        assert_eq!(config.whisper.model_path, "models/ggml-large-v3-turbo-q5_0.bin");
        assert_eq!(config.whisper.default_model, "large-q5_0");
        assert_eq!(config.whisper.language, "auto");
        assert_eq!(config.whisper.enable_gpu, true);

        // オーディオ設定
        assert_eq!(config.audio.sample_rate, 16000);
        assert_eq!(config.audio.channels, 1);
        assert_eq!(config.audio.buffer_size, 4096);

        // パフォーマンス設定
        assert_eq!(config.performance.audio_threads, 10);
        assert_eq!(config.performance.whisper_threads, 14);
        assert_eq!(config.performance.max_concurrent_requests, 10);
        assert_eq!(config.performance.request_timeout_seconds, 300);

        // 制限設定
        assert_eq!(config.limits.max_file_size_mb, 50);
        assert_eq!(config.limits.max_audio_duration_minutes, 180);
        assert_eq!(config.limits.cleanup_temp_files_after_minutes, 60);
    }

    /// 設定ファイルの読み書きテスト
    #[test]
    fn test_config_load_and_save() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        let original_config = Config::default();

        // 設定ファイルの保存
        original_config.save_to_file(&config_path).unwrap();
        assert!(config_path.exists());

        // 設定ファイルの読み込み
        let loaded_config = Config::load_from_file(&config_path).unwrap();

        // 設定値が一致することを確認
        assert_eq!(original_config.server.host, loaded_config.server.host);
        assert_eq!(original_config.server.port, loaded_config.server.port);
        assert_eq!(original_config.whisper.enable_gpu, loaded_config.whisper.enable_gpu);
        assert_eq!(original_config.audio.sample_rate, loaded_config.audio.sample_rate);
    }

    /// 不正な設定ファイルの処理テスト
    #[test]
    fn test_config_invalid_file() {
        let temp_dir = TempDir::new().unwrap();
        let invalid_config_path = temp_dir.path().join("invalid_config.toml");

        // 不正なTOMLファイルを作成
        fs::write(&invalid_config_path, "invalid toml content [[[").unwrap();

        // 読み込みが失敗することを確認
        let result = Config::load_from_file(&invalid_config_path);
        assert!(result.is_err());
    }

    /// load_or_create_defaultのテスト（ファイルが存在しない場合）
    #[test]
    fn test_config_load_or_create_default_new_file() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("new_config.toml");

        assert!(!config_path.exists());

        // ファイルが存在しない場合、デフォルト設定で作成される
        let config = Config::load_or_create_default(&config_path).unwrap();

        assert!(config_path.exists());
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.whisper.enable_gpu, true);
    }

    /// load_or_create_defaultのテスト（ファイルが存在する場合）
    #[test]
    fn test_config_load_or_create_default_existing_file() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("existing_config.toml");

        // カスタム設定を作成して保存
        let mut custom_config = Config::default();
        custom_config.server.port = 9090;
        custom_config.whisper.enable_gpu = false;
        custom_config.save_to_file(&config_path).unwrap();

        // 既存ファイルが読み込まれることを確認
        let loaded_config = Config::load_or_create_default(&config_path).unwrap();

        assert_eq!(loaded_config.server.port, 9090);
        assert_eq!(loaded_config.whisper.enable_gpu, false);
    }

    /// バリデーションテスト - 正常な設定
    #[test]
    fn test_config_validate_success() {
        let temp_dir = TempDir::new().unwrap();

        let mut config = Config::default();
        // テスト用の一時ディレクトリを設定
        let models_dir = temp_dir.path().join("models");
        let temp_work_dir = temp_dir.path().join("temp");
        let upload_dir = temp_dir.path().join("uploads");

        fs::create_dir_all(&models_dir).unwrap();
        fs::create_dir_all(&temp_work_dir).unwrap();
        fs::create_dir_all(&upload_dir).unwrap();

        // テスト用モデルファイルを作成
        let model_file = models_dir.join("test_model.bin");
        fs::write(&model_file, b"dummy model data").unwrap();

        config.paths.models_dir = models_dir.to_string_lossy().to_string();
        config.paths.temp_dir = temp_work_dir.to_string_lossy().to_string();
        config.paths.upload_dir = upload_dir.to_string_lossy().to_string();
        config.whisper.model_path = model_file.to_string_lossy().to_string();

        // バリデーションが成功することを確認
        let result = config.validate();
        assert!(result.is_ok());
    }

    /// バリデーションテスト - 無効なポート番号
    #[test]
    fn test_config_validate_invalid_port() {
        let mut config = Config::default();
        config.server.port = 0;

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("無効なポート番号"));
    }

    /// バリデーションテスト - モデルファイルが存在しない
    #[test]
    fn test_config_validate_missing_model() {
        let mut config = Config::default();
        config.whisper.model_path = "/nonexistent/model.bin".to_string();

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Whisperモデルファイルが見つかりません"));
    }

    /// バリデーションテスト - ゼロスレッド数
    #[test]
    fn test_config_validate_zero_threads() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = Config::default();

        // ディレクトリを作成
        let models_dir = temp_dir.path().join("models");
        fs::create_dir_all(&models_dir).unwrap();
        let model_file = models_dir.join("test_model.bin");
        fs::write(&model_file, b"dummy").unwrap();

        config.paths.models_dir = models_dir.to_string_lossy().to_string();
        config.whisper.model_path = model_file.to_string_lossy().to_string();
        config.performance.whisper_threads = 0;

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Whisperスレッド数は1以上である必要があります"));
    }

    /// バリデーションテスト - ゼロ同時リクエスト数
    #[test]
    fn test_config_validate_zero_concurrent_requests() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = Config::default();

        // ディレクトリを作成
        let models_dir = temp_dir.path().join("models");
        fs::create_dir_all(&models_dir).unwrap();
        let model_file = models_dir.join("test_model.bin");
        fs::write(&model_file, b"dummy").unwrap();

        config.paths.models_dir = models_dir.to_string_lossy().to_string();
        config.whisper.model_path = model_file.to_string_lossy().to_string();
        config.performance.max_concurrent_requests = 0;

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("最大同時リクエスト数は1以上である必要があります"));
    }

    /// バリデーションテスト - ゼロファイルサイズ制限
    #[test]
    fn test_config_validate_zero_file_size() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = Config::default();

        // ディレクトリを作成
        let models_dir = temp_dir.path().join("models");
        fs::create_dir_all(&models_dir).unwrap();
        let model_file = models_dir.join("test_model.bin");
        fs::write(&model_file, b"dummy").unwrap();

        config.paths.models_dir = models_dir.to_string_lossy().to_string();
        config.whisper.model_path = model_file.to_string_lossy().to_string();
        config.limits.max_file_size_mb = 0;

        let result = config.validate();
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("最大ファイルサイズは1MB以上である必要があります"));
    }

    /// ヘルパーメソッドのテスト
    #[test]
    fn test_config_helper_methods() {
        let config = Config::default();

        // server_addressメソッドのテスト
        assert_eq!(config.server_address(), "0.0.0.0:8080");

        // max_file_size_bytesメソッドのテスト
        assert_eq!(config.max_file_size_bytes(), 50 * 1024 * 1024);
    }

    /// カスタム設定値のテスト
    #[test]
    fn test_config_custom_values() {
        let mut config = Config::default();

        // カスタム値を設定
        config.server.host = "127.0.0.1".to_string();
        config.server.port = 3000;
        config.whisper.language = "ja".to_string();
        config.whisper.enable_gpu = false;
        config.audio.sample_rate = 44100;
        config.performance.whisper_threads = 8;
        config.limits.max_file_size_mb = 100;

        // カスタム値が反映されることを確認
        assert_eq!(config.server.host, "127.0.0.1");
        assert_eq!(config.server.port, 3000);
        assert_eq!(config.whisper.language, "ja");
        assert_eq!(config.whisper.enable_gpu, false);
        assert_eq!(config.audio.sample_rate, 44100);
        assert_eq!(config.performance.whisper_threads, 8);
        assert_eq!(config.limits.max_file_size_mb, 100);

        // ヘルパーメソッドも更新されることを確認
        assert_eq!(config.server_address(), "127.0.0.1:3000");
        assert_eq!(config.max_file_size_bytes(), 100 * 1024 * 1024);
    }

    /// TOMLシリアライゼーション/デシリアライゼーションのテスト
    #[test]
    fn test_config_toml_serialization() {
        let original_config = Config::default();

        // TOMLにシリアライズ
        let toml_string = toml::to_string_pretty(&original_config).unwrap();
        assert!(!toml_string.is_empty());
        assert!(toml_string.contains("[server]"));
        assert!(toml_string.contains("[whisper]"));
        assert!(toml_string.contains("[audio]"));
        assert!(toml_string.contains("[performance]"));
        assert!(toml_string.contains("[paths]"));
        assert!(toml_string.contains("[limits]"));

        // TOMLからデシリアライズ
        let deserialized_config: Config = toml::from_str(&toml_string).unwrap();

        // 元の設定と一致することを確認
        assert_eq!(original_config.server.host, deserialized_config.server.host);
        assert_eq!(original_config.whisper.enable_gpu, deserialized_config.whisper.enable_gpu);
        assert_eq!(original_config.audio.sample_rate, deserialized_config.audio.sample_rate);
    }
}

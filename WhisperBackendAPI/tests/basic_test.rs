// 基本的なモジュール機能のテスト（whisper-rsに依存しない）

use std::fs;
use tempfile::TempDir;
use WhisperBackendAPI::{config::Config, models::*};

#[cfg(test)]
mod basic_tests {
    use super::*;

    /// 設定機能の基本テスト
    #[test]
    fn test_config_basic() {
        // デフォルト設定
        let config = Config::default();
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.whisper.enable_gpu, true);
        assert_eq!(config.audio.sample_rate, 16000);

        // アドレス生成
        assert_eq!(config.server_address(), "0.0.0.0:8080");

        // ファイルサイズ計算
        assert_eq!(config.max_file_size_bytes(), 50 * 1024 * 1024);
    }

    /// 設定ファイルの読み書きテスト
    #[test]
    fn test_config_file_operations() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test_config.toml");

        let original_config = Config::default();

        // 保存
        original_config.save_to_file(&config_path).unwrap();
        assert!(config_path.exists());

        // 読み込み
        let loaded_config = Config::load_from_file(&config_path).unwrap();
        assert_eq!(loaded_config.server.port, original_config.server.port);
        assert_eq!(
            loaded_config.whisper.enable_gpu,
            original_config.whisper.enable_gpu
        );
    }

    /// TranscriptionSegmentのテスト
    #[test]
    fn test_transcription_segment() {
        let segment = TranscriptionSegment::new(
            "Hello world".to_string(),
            1000, // 1秒
            3000, // 3秒
        );

        assert_eq!(segment.text, "Hello world");
        assert_eq!(segment.start_time_ms, 1000);
        assert_eq!(segment.end_time_ms, 3000);
        assert_eq!(segment.duration_ms(), 2000);

        // SRT形式
        let srt = segment.to_srt_format(0);
        assert!(srt.contains("1"));
        assert!(srt.contains("00:00:01,000 --> 00:00:03,000"));
        assert!(srt.contains("Hello world"));

        // VTT形式
        let vtt = segment.to_vtt_format();
        assert!(vtt.contains("00:00:01.000 --> 00:00:03.000"));
        assert!(vtt.contains("Hello world"));
    }

    /// ServerStatsのテスト
    #[test]
    fn test_server_stats() {
        let mut stats = ServerStats::default();

        // 初期状態
        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.successful_transcriptions, 0);
        assert_eq!(stats.failed_transcriptions, 0);
        assert_eq!(stats.success_rate(), 0.0);

        // リクエスト記録
        stats.record_request();
        stats.record_success(1500, Some(60_000));

        assert_eq!(stats.total_requests, 1);
        assert_eq!(stats.successful_transcriptions, 1);
        assert_eq!(stats.average_processing_time_ms, 1500.0);
        assert_eq!(stats.average_processing_time_one_minute_ms, 1500.0);
        assert_eq!(
            stats.average_processing_time_one_minute_display,
            "1.50 s/min"
        );
        assert_eq!(stats.success_rate(), 100.0);

        // 失敗記録
        stats.record_request();
        stats.record_failure();

        assert_eq!(stats.total_requests, 2);
        assert_eq!(stats.failed_transcriptions, 1);
        assert_eq!(stats.success_rate(), 50.0);
    }

    /// ModelCatalogのテスト
    #[test]
    fn test_model_catalog() {
        let catalog = ModelCatalog::default();

        // 期待されるモデルが存在する
        assert!(catalog.models.contains_key("tiny"));
        assert!(catalog.models.contains_key("base"));
        assert!(catalog.models.contains_key("small"));
        assert!(catalog.models.contains_key("medium"));
        assert!(catalog.models.contains_key("large-v3-turbo-q5_0"));

        // モデル定義の内容確認
        let tiny_model = &catalog.models["tiny"];
        assert_eq!(tiny_model.name, "Whisper Tiny");
        assert_eq!(tiny_model.size_mb, 39);
        assert!(tiny_model.download_url.starts_with("https://"));
        assert_eq!(tiny_model.language_support, vec!["multilingual"]);
    }

    /// ApiErrorCodeのテスト
    #[test]
    fn test_api_error_code() {
        assert_eq!(ApiErrorCode::InvalidInput.as_str(), "INVALID_INPUT");
        assert_eq!(ApiErrorCode::FileTooLarge.as_str(), "FILE_TOO_LARGE");
        assert_eq!(
            ApiErrorCode::UnsupportedFormat.as_str(),
            "UNSUPPORTED_FORMAT"
        );
        assert_eq!(ApiErrorCode::ProcessingFailed.as_str(), "PROCESSING_FAILED");
        assert_eq!(ApiErrorCode::ModelNotLoaded.as_str(), "MODEL_NOT_LOADED");
        assert_eq!(ApiErrorCode::ServerOverloaded.as_str(), "SERVER_OVERLOADED");
        assert_eq!(ApiErrorCode::InternalError.as_str(), "INTERNAL_ERROR");
    }

    /// ModelQualityのテスト
    #[test]
    fn test_model_quality() {
        assert_eq!(ModelQuality::Tiny.to_string(), "tiny");
        assert_eq!(ModelQuality::Base.to_string(), "base");
        assert_eq!(ModelQuality::Small.to_string(), "small");
        assert_eq!(ModelQuality::Medium.to_string(), "medium");
        assert_eq!(ModelQuality::Large.to_string(), "large");
        assert_eq!(ModelQuality::LargeV2.to_string(), "large-v2");
        assert_eq!(ModelQuality::LargeV3.to_string(), "large-v3");
    }

    /// JSON シリアライゼーション/デシリアライゼーション
    #[test]
    fn test_json_serialization() {
        // TranscriptionSegment
        let segment = TranscriptionSegment::new("Test text".to_string(), 1000, 2000);
        let json = serde_json::to_string(&segment).unwrap();
        let deserialized: TranscriptionSegment = serde_json::from_str(&json).unwrap();
        assert_eq!(segment.text, deserialized.text);
        assert_eq!(segment.start_time_ms, deserialized.start_time_ms);

        // ServerStats
        let mut stats = ServerStats::default();
        stats.record_request();
        stats.record_success(1500, Some(60_000));
        let json = serde_json::to_string(&stats).unwrap();
        let deserialized: ServerStats = serde_json::from_str(&json).unwrap();
        assert_eq!(stats.total_requests, deserialized.total_requests);
        assert_eq!(
            stats.successful_transcriptions,
            deserialized.successful_transcriptions
        );
        assert_eq!(
            stats.average_processing_time_ms,
            deserialized.average_processing_time_ms
        );
        assert_eq!(
            stats.average_processing_time_one_minute_ms,
            deserialized.average_processing_time_one_minute_ms
        );
        assert_eq!(
            stats.average_processing_time_one_minute_display,
            deserialized.average_processing_time_one_minute_display
        );

        // ModelCatalog
        let catalog = ModelCatalog::default();
        let json = serde_json::to_string(&catalog).unwrap();
        let deserialized: ModelCatalog = serde_json::from_str(&json).unwrap();
        assert_eq!(catalog.models.len(), deserialized.models.len());
    }

    /// 設定バリデーションのテスト
    #[test]
    fn test_config_validation() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = Config::default();

        // 正常なバリデーション用のセットアップ
        let models_dir = temp_dir.path().join("models");
        let temp_work_dir = temp_dir.path().join("temp");
        let upload_dir = temp_dir.path().join("uploads");

        fs::create_dir_all(&models_dir).unwrap();
        fs::create_dir_all(&temp_work_dir).unwrap();
        fs::create_dir_all(&upload_dir).unwrap();

        let model_file = models_dir.join("test_model.bin");
        fs::write(&model_file, b"dummy model data").unwrap();

        config.paths.models_dir = models_dir.to_string_lossy().to_string();
        config.paths.temp_dir = temp_work_dir.to_string_lossy().to_string();
        config.paths.upload_dir = upload_dir.to_string_lossy().to_string();
        config.whisper.model_path = model_file.to_string_lossy().to_string();

        // 正常なバリデーション
        assert!(config.validate().is_ok());

        // エラーケースのテスト
        config.server.port = 0;
        assert!(config.validate().is_err());

        config.server.port = 8080;
        config.performance.whisper_threads = 0;
        assert!(config.validate().is_err());

        config.performance.whisper_threads = 4;
        config.limits.max_file_size_mb = 0;
        assert!(config.validate().is_err());
    }

    /// 音声関連のユーティリティテスト
    #[test]
    fn test_audio_utilities() {
        use WhisperBackendAPI::audio::format_file_size;

        assert_eq!(format_file_size(0), "0 B");
        assert_eq!(format_file_size(512), "512 B");
        assert_eq!(format_file_size(1024), "1.0 KB");
        assert_eq!(format_file_size(1536), "1.5 KB");
        assert_eq!(format_file_size(1048576), "1.0 MB");
        assert_eq!(format_file_size(1073741824), "1.0 GB");
    }

    /// 時間フォーマット詳細テスト
    #[test]
    fn test_time_formatting() {
        let test_cases = vec![
            (0, "00:00:00,000", "00:00:00.000"),
            (500, "00:00:00,500", "00:00:00.500"),
            (1000, "00:00:01,000", "00:00:01.000"),
            (61500, "00:01:01,500", "00:01:01.500"),
            (3661500, "01:01:01,500", "01:01:01.500"),
        ];

        for (ms, expected_srt, expected_vtt) in test_cases {
            let segment = TranscriptionSegment::new("Test".to_string(), ms, ms);
            let srt = segment.to_srt_format(0);
            let vtt = segment.to_vtt_format();

            assert!(
                srt.contains(expected_srt),
                "SRT format test failed for {}ms",
                ms
            );
            assert!(
                vtt.contains(expected_vtt),
                "VTT format test failed for {}ms",
                ms
            );
        }
    }
}

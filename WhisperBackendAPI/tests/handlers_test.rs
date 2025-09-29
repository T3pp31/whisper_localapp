use axum::{
    body::Body,
    http::{Request, StatusCode},
    response::Response,
};
use serde_json::Value;
use std::fs;
use tempfile::TempDir;
use tower::ServiceExt;
use WhisperBackendAPI::{
    config::Config,
    handlers::{ApiError, AppState},
    models::{ApiErrorCode, *},
    whisper::WhisperEngine,
};

#[cfg(test)]
mod handlers_tests {
    use super::*;

    /// テスト用のAppStateを作成
    fn create_test_app_state(temp_dir: &TempDir) -> AppState {
        let mut config = Config::default();

        // テスト用のディレクトリを設定
        let models_dir = temp_dir.path().join("models");
        fs::create_dir_all(&models_dir).unwrap();

        config.paths.models_dir = models_dir.to_string_lossy().to_string();
        config.paths.temp_dir = temp_dir.path().to_string_lossy().to_string();
        config.paths.upload_dir = temp_dir.path().to_string_lossy().to_string();
        config.limits.max_file_size_mb = 10;
        config.limits.max_audio_duration_minutes = 5;
        config.server.host = "127.0.0.1".to_string();
        config.server.port = 8080;

        AppState::new(config)
    }

    /// AppStateのテスト
    mod app_state_tests {
        use super::*;

        #[test]
        fn test_app_state_new() {
            let temp_dir = TempDir::new().unwrap();
            let app_state = create_test_app_state(&temp_dir);

            assert_eq!(app_state.config.server.host, "127.0.0.1");
            assert_eq!(app_state.config.server.port, 8080);

            // WhisperEngineは初期状態ではNone
            let engine_guard = app_state.whisper_engine.lock().unwrap();
            assert!(engine_guard.is_none());
        }

        #[test]
        fn test_app_state_with_whisper_engine() {
            let temp_dir = TempDir::new().unwrap();
            let mut config = Config::default();

            // ダミーモデルファイルを作成
            let models_dir = temp_dir.path().join("models");
            fs::create_dir_all(&models_dir).unwrap();
            let model_file = models_dir.join("test_model.bin");
            fs::write(&model_file, b"dummy model").unwrap();

            config.whisper.model_path = model_file.to_string_lossy().to_string();
            config.whisper.enable_gpu = false;

            let app_state = AppState::new(config.clone());

            // この段階ではwhisper_engineはないが、実際の実装では
            // WhisperEngine::newが成功した場合にwith_whisper_engineが呼ばれる

            // モックのWhisperEngineを作成（実際のテストでは実際のエンジンは使用しない）
            // ここではAppStateの構造をテストするだけ
            assert!(app_state
                .config
                .whisper
                .model_path
                .contains("test_model.bin"));
        }

        #[test]
        fn test_app_state_clone() {
            let temp_dir = TempDir::new().unwrap();
            let app_state = create_test_app_state(&temp_dir);

            let cloned = app_state.clone();

            // Configの値が同じであることを確認
            assert_eq!(app_state.config.server.host, cloned.config.server.host);
            assert_eq!(app_state.config.server.port, cloned.config.server.port);

            // 開始時刻が同じであることを確認（同じArcを参照）
            let original_time = app_state.start_time.elapsed();
            let cloned_time = cloned.start_time.elapsed();
            // 時間差が1ms以内であることを確認
            assert!(
                (original_time.as_millis() as i128 - cloned_time.as_millis() as i128).abs() < 2
            );
        }
    }

    /// ApiErrorのテスト
    mod api_error_tests {
        use super::*;

        #[test]
        fn test_api_error_new() {
            let error = ApiError::new(ApiErrorCode::InvalidInput, "Test error message");

            assert!(matches!(error.code, ApiErrorCode::InvalidInput));
            assert_eq!(error.message, "Test error message");
            assert!(error.details.is_none());
        }

        #[test]
        fn test_api_error_with_details() {
            let error = ApiError::new(ApiErrorCode::ProcessingFailed, "Failed to process")
                .with_details("Stack trace or additional info");

            assert!(matches!(error.code, ApiErrorCode::ProcessingFailed));
            assert_eq!(error.message, "Failed to process");
            assert_eq!(
                error.details,
                Some("Stack trace or additional info".to_string())
            );
        }

        #[test]
        fn test_api_error_from_anyhow() {
            let anyhow_error = anyhow::anyhow!("Something went wrong");
            let api_error = ApiError::from(anyhow_error);

            assert!(matches!(api_error.code, ApiErrorCode::InternalError));
            assert_eq!(api_error.message, "Something went wrong");
        }

        #[test]
        fn test_api_error_debug() {
            let error = ApiError::new(ApiErrorCode::FileTooLarge, "File is too big");
            let debug_str = format!("{:?}", error);

            assert!(debug_str.contains("FileTooLarge"));
            assert!(debug_str.contains("File is too big"));
        }
    }

    /// ユーティリティ関数のテスト
    mod utility_tests {
        use super::*;

        #[cfg(target_os = "linux")]
        #[test]
        fn test_get_memory_usage_mb() {
            use WhisperBackendAPI::handlers::get_memory_usage_mb;

            // Linux環境でのメモリ使用量取得テスト
            // 実際の値は取得できないかもしれないが、関数が実行できることを確認
            let memory_usage = get_memory_usage_mb();

            // 結果が何らかの値を持つかもしれないし、Noneかもしれない
            // エラーが発生しないことが重要
            match memory_usage {
                Some(mb) => assert!(mb > 0), // 何らかの正の値
                None => {}                   // 取得できない場合もある
            }
        }

        #[cfg(not(target_os = "linux"))]
        #[test]
        fn test_get_memory_usage_mb_non_linux() {
            use WhisperBackendAPI::handlers::get_memory_usage_mb;

            // Linux以外の環境では常にNoneを返す
            let memory_usage = get_memory_usage_mb();
            assert!(memory_usage.is_none());
        }
    }

    /// GPUステータス関連のテスト
    mod gpu_status_tests {
        use super::*;
        use WhisperBackendAPI::handlers::{
            detect_gpu_libraries, generate_gpu_recommendations, GpuEnvironmentInfo, GpuLibraryInfo,
            GpuStatusResponse,
        };

        #[test]
        fn test_detect_gpu_libraries() {
            let gpu_info = detect_gpu_libraries();

            // 構造体が適切に初期化されていることを確認
            assert!(!gpu_info.detection_notes.is_empty());

            // プラットフォームに関係なく何らかの情報が返されることを確認
            if cfg!(target_os = "linux") {
                // Linuxでは具体的なライブラリ検出を試行
                assert!(gpu_info
                    .detection_notes
                    .iter()
                    .any(|note| note.contains("CUDA") || note.contains("OpenCL")));
            } else {
                // Linux以外では未実装メッセージが含まれる
                assert!(gpu_info
                    .detection_notes
                    .iter()
                    .any(|note| note.contains("not implemented for this platform")));
            }
        }

        #[test]
        fn test_generate_gpu_recommendations_gpu_disabled() {
            let temp_dir = TempDir::new().unwrap();
            let app_state = create_test_app_state(&temp_dir);

            let recommendations = generate_gpu_recommendations(&app_state.config, false);

            assert!(!recommendations.is_empty());
            assert!(recommendations
                .iter()
                .any(|rec| rec.contains("CPU処理で動作しています")));
        }

        #[test]
        fn test_generate_gpu_recommendations_gpu_enabled_but_not_working() {
            let temp_dir = TempDir::new().unwrap();
            let mut config = Config::default();
            config.whisper.enable_gpu = true;

            let recommendations = generate_gpu_recommendations(&config, false);

            assert!(!recommendations.is_empty());
            assert!(
                recommendations
                    .iter()
                    .any(|rec| rec
                        .contains("GPUが設定で有効化されているが実際には使用されていません"))
            );
            assert!(recommendations
                .iter()
                .any(|rec| rec.contains("リビルドしてください")));
        }

        #[test]
        fn test_generate_gpu_recommendations_gpu_working() {
            let temp_dir = TempDir::new().unwrap();
            let mut config = Config::default();
            config.whisper.enable_gpu = true;

            let recommendations = generate_gpu_recommendations(&config, true);

            assert!(!recommendations.is_empty());
            assert!(recommendations
                .iter()
                .any(|rec| rec.contains("GPU加速が正常に有効化されています")));
        }

        #[test]
        fn test_gpu_status_response_serialization() {
            use WhisperBackendAPI::whisper::ModelInfo;

            let model_info = ModelInfo {
                is_loaded: true,
                language: Some("ja".to_string()),
                threads: 4,
                enable_gpu: false,
            };

            let response = GpuStatusResponse {
                gpu_enabled_in_config: true,
                gpu_actually_enabled: false,
                model_info: Some(model_info),
                environment: GpuEnvironmentInfo {
                    whisper_cublas: false,
                    whisper_opencl: false,
                    cuda_path: None,
                    cuda_feature_enabled: false,
                    opencl_feature_enabled: false,
                },
                gpu_library_info: GpuLibraryInfo {
                    cuda_runtime_detected: false,
                    cublas_detected: false,
                    opencl_detected: false,
                    detection_notes: vec!["Test note".to_string()],
                },
                recommendations: vec!["Test recommendation".to_string()],
            };

            // JSONシリアライゼーションが動作することを確認
            let json = serde_json::to_string(&response).unwrap();
            assert!(!json.is_empty());
            assert!(json.contains("gpu_enabled_in_config"));
            assert!(json.contains("gpu_actually_enabled"));
            assert!(json.contains("Test recommendation"));
        }
    }

    /// 統計機能のテスト
    mod stats_tests {
        use super::*;

        #[test]
        fn test_server_stats_initialization() {
            let temp_dir = TempDir::new().unwrap();
            let app_state = create_test_app_state(&temp_dir);

            let stats = app_state.stats.lock().unwrap();
            assert_eq!(stats.total_requests, 0);
            assert_eq!(stats.successful_transcriptions, 0);
            assert_eq!(stats.failed_transcriptions, 0);
            assert_eq!(stats.average_processing_time_ms, 0.0);
            assert_eq!(stats.average_processing_time_one_minute_ms, 0.0);
            assert_eq!(
                stats.average_processing_time_one_minute_display,
                "0.00 s/min"
            );
        }

        #[test]
        fn test_server_stats_recording() {
            let temp_dir = TempDir::new().unwrap();
            let app_state = create_test_app_state(&temp_dir);

            {
                let mut stats = app_state.stats.lock().unwrap();
                stats.record_request();
                stats.record_success(1500, Some(60_000));
            }

            let stats = app_state.stats.lock().unwrap();
            assert_eq!(stats.total_requests, 1);
            assert_eq!(stats.successful_transcriptions, 1);
            assert_eq!(stats.total_processing_time_ms, 1500);
            assert_eq!(stats.average_processing_time_ms, 1500.0);
            assert_eq!(stats.average_processing_time_one_minute_ms, 1500.0);
            assert_eq!(
                stats.average_processing_time_one_minute_display,
                "1.50 s/min"
            );
            assert_eq!(stats.success_rate(), 100.0);
        }

        #[test]
        fn test_server_stats_mixed_results() {
            let temp_dir = TempDir::new().unwrap();
            let app_state = create_test_app_state(&temp_dir);

            {
                let mut stats = app_state.stats.lock().unwrap();
                // 2回リクエスト、1回成功、1回失敗
                stats.record_request();
                stats.record_success(1000, Some(60_000));

                stats.record_request();
                stats.record_failure();
            }

            let stats = app_state.stats.lock().unwrap();
            assert_eq!(stats.total_requests, 2);
            assert_eq!(stats.successful_transcriptions, 1);
            assert_eq!(stats.failed_transcriptions, 1);
            assert_eq!(stats.success_rate(), 50.0);
        }
    }

    /// CORS関連のテスト
    mod cors_tests {
        use super::*;

        #[tokio::test]
        async fn test_add_cors_headers() {
            use axum::response::IntoResponse;
            use WhisperBackendAPI::handlers::add_cors_headers;

            let response = add_cors_headers().await;
            let response = response.into_response();

            assert_eq!(response.status(), StatusCode::OK);

            // ヘッダーの確認は実際のHTTPレスポンスからは困難だが
            // 関数が正常に実行されることを確認
            assert_eq!(response.status(), StatusCode::OK);
        }
    }

    /// モデル関連のテスト
    mod model_tests {
        use super::*;

        #[test]
        fn test_model_catalog_access() {
            let catalog = ModelCatalog::default();

            // デフォルトカタログが適切に初期化されていることを確認
            assert!(!catalog.models.is_empty());
            assert!(catalog.models.contains_key("tiny"));
            assert!(catalog.models.contains_key("base"));
            assert!(catalog.models.contains_key("small"));
        }

        #[test]
        fn test_model_info_creation() {
            let model_def = &ModelCatalog::default().models["base"];

            let model_info = ModelInfo {
                name: model_def.name.clone(),
                file_path: "/test/path/ggml-base.bin".to_string(),
                size_mb: model_def.size_mb,
                description: model_def.description.clone(),
                language_support: model_def.language_support.clone(),
                is_available: false,
            };

            assert_eq!(model_info.name, "Whisper Base");
            assert_eq!(model_info.size_mb, 142);
            assert!(!model_info.is_available);
            assert!(model_info
                .language_support
                .contains(&"multilingual".to_string()));
        }
    }

    /// エラーレスポンス変換のテスト
    mod error_response_tests {
        use super::*;
        use axum::response::IntoResponse;

        #[test]
        fn test_api_error_to_response() {
            let error = ApiError::new(ApiErrorCode::InvalidInput, "Invalid input data")
                .with_details("Missing required field");

            let response = error.into_response();
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        }

        #[test]
        fn test_all_error_codes_status_mapping() {
            let error_codes_and_statuses = vec![
                (ApiErrorCode::InvalidInput, StatusCode::BAD_REQUEST),
                (ApiErrorCode::FileTooLarge, StatusCode::PAYLOAD_TOO_LARGE),
                (
                    ApiErrorCode::UnsupportedFormat,
                    StatusCode::UNSUPPORTED_MEDIA_TYPE,
                ),
                (
                    ApiErrorCode::ProcessingFailed,
                    StatusCode::INTERNAL_SERVER_ERROR,
                ),
                (
                    ApiErrorCode::ModelNotLoaded,
                    StatusCode::SERVICE_UNAVAILABLE,
                ),
                (
                    ApiErrorCode::ServerOverloaded,
                    StatusCode::TOO_MANY_REQUESTS,
                ),
                (
                    ApiErrorCode::InternalError,
                    StatusCode::INTERNAL_SERVER_ERROR,
                ),
            ];

            for (error_code, expected_status) in error_codes_and_statuses {
                let error = ApiError::new(error_code, "Test message");
                let response = error.into_response();
                assert_eq!(response.status(), expected_status);
            }
        }
    }

    /// ファイル処理関連のテスト
    mod file_processing_tests {
        use super::*;

        #[test]
        fn test_file_size_validation() {
            let temp_dir = TempDir::new().unwrap();
            let app_state = create_test_app_state(&temp_dir);

            let max_size = app_state.config.max_file_size_bytes();
            assert_eq!(max_size, 10 * 1024 * 1024); // 10MB
        }

        #[test]
        fn test_supported_formats_config() {
            let temp_dir = TempDir::new().unwrap();
            let app_state = create_test_app_state(&temp_dir);

            let supported_formats = &app_state.config.audio.supported_formats;
            assert!(supported_formats.contains(&"wav".to_string()));
            assert!(supported_formats.contains(&"mp3".to_string()));
            assert!(supported_formats.contains(&"m4a".to_string()));
            assert!(supported_formats.contains(&"flac".to_string()));
            assert!(supported_formats.contains(&"ogg".to_string()));
        }

        #[test]
        fn test_audio_duration_limits() {
            let temp_dir = TempDir::new().unwrap();
            let app_state = create_test_app_state(&temp_dir);

            assert_eq!(app_state.config.limits.max_audio_duration_minutes, 5);
            assert_eq!(app_state.config.limits.max_file_size_mb, 10);
        }
    }

    /// タイムアウト・制限のテスト
    mod limits_tests {
        use super::*;

        #[test]
        fn test_request_timeout_configuration() {
            let temp_dir = TempDir::new().unwrap();
            let app_state = create_test_app_state(&temp_dir);

            assert_eq!(app_state.config.performance.request_timeout_seconds, 300); // 5分
            assert_eq!(app_state.config.performance.max_concurrent_requests, 10);
        }

        #[test]
        fn test_performance_configuration() {
            let temp_dir = TempDir::new().unwrap();
            let app_state = create_test_app_state(&temp_dir);

            assert_eq!(app_state.config.performance.whisper_threads, 14);
            assert_eq!(app_state.config.performance.audio_threads, 10);
        }

        #[test]
        fn test_cleanup_configuration() {
            let temp_dir = TempDir::new().unwrap();
            let app_state = create_test_app_state(&temp_dir);

            assert_eq!(app_state.config.limits.cleanup_temp_files_after_minutes, 60);
        }
    }
}

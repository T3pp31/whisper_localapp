use WhisperBackendAPI::models::*;

#[cfg(test)]
mod models_tests {
    use super::*;

    /// TranscriptionSegmentのテスト
    mod transcription_segment_tests {
        use super::*;

        #[test]
        fn test_transcription_segment_new() {
            let segment = TranscriptionSegment::new(
                "Hello world".to_string(),
                1000, // 1秒
                3000, // 3秒
            );

            assert_eq!(segment.text, "Hello world");
            assert_eq!(segment.start_time_ms, 1000);
            assert_eq!(segment.end_time_ms, 3000);
        }

        #[test]
        fn test_duration_ms() {
            let segment = TranscriptionSegment::new(
                "Test".to_string(),
                1500, // 1.5秒
                4500, // 4.5秒
            );

            assert_eq!(segment.duration_ms(), 3000); // 3秒
        }

        #[test]
        fn test_duration_ms_zero() {
            let segment = TranscriptionSegment::new(
                "Test".to_string(),
                2000,
                2000, // 同じ時間
            );

            assert_eq!(segment.duration_ms(), 0);
        }

        #[test]
        fn test_duration_ms_saturating_sub() {
            // end_time < start_timeの場合（通常は起こらないが安全性テスト）
            let segment = TranscriptionSegment::new(
                "Test".to_string(),
                5000,
                3000,
            );

            assert_eq!(segment.duration_ms(), 0); // saturating_subにより0になる
        }

        #[test]
        fn test_to_srt_format() {
            let segment = TranscriptionSegment::new(
                "Hello, world!".to_string(),
                1500,  // 00:00:01,500
                4250,  // 00:00:04,250
            );

            let srt = segment.to_srt_format(0); // index 0 -> SRTでは1番目
            let lines: Vec<&str> = srt.trim().split('\n').collect();

            assert_eq!(lines[0], "1"); // SRT番号
            assert_eq!(lines[1], "00:00:01,500 --> 00:00:04,250"); // 時間
            assert_eq!(lines[2], "Hello, world!"); // テキスト
        }

        #[test]
        fn test_to_srt_format_with_hours() {
            let segment = TranscriptionSegment::new(
                "Long content".to_string(),
                3661500, // 01:01:01,500
                3665250, // 01:01:05,250
            );

            let srt = segment.to_srt_format(9); // index 9 -> SRTでは10番目
            let lines: Vec<&str> = srt.trim().split('\n').collect();

            assert_eq!(lines[0], "10"); // SRT番号
            assert_eq!(lines[1], "01:01:01,500 --> 01:01:05,250"); // 時間
            assert_eq!(lines[2], "Long content"); // テキスト
        }

        #[test]
        fn test_to_vtt_format() {
            let segment = TranscriptionSegment::new(
                "VTT test".to_string(),
                2750,  // 00:00:02.750
                6100,  // 00:00:06.100
            );

            let vtt = segment.to_vtt_format();
            let lines: Vec<&str> = vtt.trim().split('\n').collect();

            assert_eq!(lines[0], "00:00:02.750 --> 00:00:06.100"); // VTT時間形式
            assert_eq!(lines[1], "VTT test"); // テキスト
        }

        #[test]
        fn test_ms_to_srt_time() {
            // プライベートメソッドの動作を public メソッド経由で確認
            let segment = TranscriptionSegment::new("Test".to_string(), 0, 0);
            let srt = segment.to_srt_format(0);

            // 各種時間形式のテスト
            let test_cases = vec![
                (0, "00:00:00,000"),
                (500, "00:00:00,500"),
                (1000, "00:00:01,000"),
                (61500, "00:01:01,500"),
                (3661500, "01:01:01,500"),
            ];

            for (ms, expected) in test_cases {
                let segment = TranscriptionSegment::new("Test".to_string(), ms, ms);
                let srt = segment.to_srt_format(0);
                assert!(srt.contains(expected));
            }
        }

        #[test]
        fn test_ms_to_vtt_time() {
            // VTT形式の時間テスト
            let test_cases = vec![
                (0, "00:00:00.000"),
                (500, "00:00:00.500"),
                (1000, "00:00:01.000"),
                (61500, "00:01:01.500"),
                (3661500, "01:01:01.500"),
            ];

            for (ms, expected) in test_cases {
                let segment = TranscriptionSegment::new("Test".to_string(), ms, ms);
                let vtt = segment.to_vtt_format();
                assert!(vtt.contains(expected));
            }
        }

        #[test]
        fn test_segment_clone() {
            let original = TranscriptionSegment::new(
                "Clone test".to_string(),
                1000,
                2000,
            );

            let cloned = original.clone();

            assert_eq!(cloned.text, original.text);
            assert_eq!(cloned.start_time_ms, original.start_time_ms);
            assert_eq!(cloned.end_time_ms, original.end_time_ms);
        }
    }

    /// ServerStatsのテスト
    mod server_stats_tests {
        use super::*;

        #[test]
        fn test_server_stats_default() {
            let stats = ServerStats::default();

            assert_eq!(stats.total_requests, 0);
            assert_eq!(stats.successful_transcriptions, 0);
            assert_eq!(stats.failed_transcriptions, 0);
            assert_eq!(stats.total_processing_time_ms, 0);
            assert_eq!(stats.average_processing_time_ms, 0.0);
            assert_eq!(stats.active_requests, 0);
            assert_eq!(stats.uptime_seconds, 0);
        }

        #[test]
        fn test_record_request() {
            let mut stats = ServerStats::default();

            stats.record_request();
            assert_eq!(stats.total_requests, 1);
            assert_eq!(stats.active_requests, 1);

            stats.record_request();
            assert_eq!(stats.total_requests, 2);
            assert_eq!(stats.active_requests, 2);
        }

        #[test]
        fn test_record_success() {
            let mut stats = ServerStats::default();
            stats.record_request();

            stats.record_success(1000);

            assert_eq!(stats.successful_transcriptions, 1);
            assert_eq!(stats.active_requests, 0); // デクリメントされる
            assert_eq!(stats.total_processing_time_ms, 1000);
            assert_eq!(stats.average_processing_time_ms, 1000.0);

            // 2回目の成功を記録
            stats.record_request();
            stats.record_success(2000);

            assert_eq!(stats.successful_transcriptions, 2);
            assert_eq!(stats.total_processing_time_ms, 3000);
            assert_eq!(stats.average_processing_time_ms, 1500.0); // (1000 + 2000) / 2
        }

        #[test]
        fn test_record_failure() {
            let mut stats = ServerStats::default();
            stats.record_request();
            stats.record_request();

            stats.record_failure();

            assert_eq!(stats.failed_transcriptions, 1);
            assert_eq!(stats.active_requests, 1); // 1つデクリメント

            stats.record_failure();

            assert_eq!(stats.failed_transcriptions, 2);
            assert_eq!(stats.active_requests, 0); // さらにデクリメント
        }

        #[test]
        fn test_record_failure_saturating_sub() {
            let mut stats = ServerStats::default();
            // active_requests = 0の状態で失敗を記録
            stats.record_failure();

            assert_eq!(stats.failed_transcriptions, 1);
            assert_eq!(stats.active_requests, 0); // 0以下にはならない
        }

        #[test]
        fn test_success_rate() {
            let mut stats = ServerStats::default();

            // リクエストなしの場合
            assert_eq!(stats.success_rate(), 0.0);

            // 100%成功の場合
            stats.record_request();
            stats.record_success(1000);
            assert_eq!(stats.success_rate(), 100.0);

            // 50%成功の場合
            stats.record_request();
            stats.record_failure();
            assert_eq!(stats.success_rate(), 50.0);

            // さらにテストを追加（25%成功）
            stats.record_request();
            stats.record_failure();
            stats.record_request();
            stats.record_failure();
            assert_eq!(stats.success_rate(), 25.0);
        }

        #[test]
        fn test_average_processing_time_with_zero_successful() {
            let mut stats = ServerStats::default();

            // 成功なしの場合は平均は0のまま
            stats.record_request();
            stats.record_failure();

            assert_eq!(stats.average_processing_time_ms, 0.0);
        }

        #[test]
        fn test_stats_clone() {
            let mut original = ServerStats::default();
            original.record_request();
            original.record_success(5000);

            let cloned = original.clone();

            assert_eq!(cloned.total_requests, original.total_requests);
            assert_eq!(cloned.successful_transcriptions, original.successful_transcriptions);
            assert_eq!(cloned.total_processing_time_ms, original.total_processing_time_ms);
            assert_eq!(cloned.average_processing_time_ms, original.average_processing_time_ms);
        }
    }

    /// ModelQualityのテスト
    mod model_quality_tests {
        use super::*;

        #[test]
        fn test_model_quality_to_string() {
            assert_eq!(ModelQuality::Tiny.to_string(), "tiny");
            assert_eq!(ModelQuality::Base.to_string(), "base");
            assert_eq!(ModelQuality::Small.to_string(), "small");
            assert_eq!(ModelQuality::Medium.to_string(), "medium");
            assert_eq!(ModelQuality::Large.to_string(), "large");
            assert_eq!(ModelQuality::LargeV2.to_string(), "large-v2");
            assert_eq!(ModelQuality::LargeV3.to_string(), "large-v3");
        }

        #[test]
        fn test_model_quality_clone() {
            let quality = ModelQuality::LargeV3;
            let cloned = quality.clone();

            assert_eq!(quality.to_string(), cloned.to_string());
        }
    }

    /// ModelCatalogのテスト
    mod model_catalog_tests {
        use super::*;

        #[test]
        fn test_model_catalog_default() {
            let catalog = ModelCatalog::default();

            // 期待されるモデルが含まれていることを確認
            assert!(catalog.models.contains_key("tiny"));
            assert!(catalog.models.contains_key("base"));
            assert!(catalog.models.contains_key("small"));
            assert!(catalog.models.contains_key("medium"));
            assert!(catalog.models.contains_key("large-v3-turbo-q5_0"));
        }

        #[test]
        fn test_model_definition_tiny() {
            let catalog = ModelCatalog::default();
            let tiny_model = catalog.models.get("tiny").unwrap();

            assert_eq!(tiny_model.name, "Whisper Tiny");
            assert_eq!(tiny_model.file_name, "ggml-tiny.bin");
            assert_eq!(tiny_model.size_mb, 39);
            assert!(tiny_model.description.contains("最小モデル"));
            assert_eq!(tiny_model.language_support, vec!["multilingual"]);
            assert!(matches!(tiny_model.quality, ModelQuality::Tiny));
        }

        #[test]
        fn test_model_definition_base() {
            let catalog = ModelCatalog::default();
            let base_model = catalog.models.get("base").unwrap();

            assert_eq!(base_model.name, "Whisper Base");
            assert_eq!(base_model.file_name, "ggml-base.bin");
            assert_eq!(base_model.size_mb, 142);
            assert!(base_model.description.contains("基本モデル"));
            assert!(base_model.download_url.contains("huggingface.co"));
            assert!(matches!(base_model.quality, ModelQuality::Base));
        }

        #[test]
        fn test_model_definition_large_v3_turbo() {
            let catalog = ModelCatalog::default();
            let large_model = catalog.models.get("large-v3-turbo-q5_0").unwrap();

            assert_eq!(large_model.name, "Whisper Large V3 Turbo Q5_0");
            assert_eq!(large_model.file_name, "ggml-large-v3-turbo-q5_0.bin");
            assert_eq!(large_model.size_mb, 809);
            assert!(large_model.description.contains("最新の大型モデル"));
            assert!(matches!(large_model.quality, ModelQuality::LargeV3));
        }

        #[test]
        fn test_all_models_have_required_fields() {
            let catalog = ModelCatalog::default();

            for (key, model) in &catalog.models {
                assert!(!model.name.is_empty(), "Model {} has empty name", key);
                assert!(!model.file_name.is_empty(), "Model {} has empty file_name", key);
                assert!(!model.download_url.is_empty(), "Model {} has empty download_url", key);
                assert!(model.size_mb > 0, "Model {} has zero size", key);
                assert!(!model.description.is_empty(), "Model {} has empty description", key);
                assert!(!model.language_support.is_empty(), "Model {} has empty language_support", key);
                assert!(model.download_url.starts_with("https://"), "Model {} has invalid download_url", key);
            }
        }

        #[test]
        fn test_model_catalog_clone() {
            let original = ModelCatalog::default();
            let cloned = original.clone();

            assert_eq!(original.models.len(), cloned.models.len());

            for key in original.models.keys() {
                assert!(cloned.models.contains_key(key));
                let orig_model = original.models.get(key).unwrap();
                let cloned_model = cloned.models.get(key).unwrap();
                assert_eq!(orig_model.name, cloned_model.name);
                assert_eq!(orig_model.size_mb, cloned_model.size_mb);
            }
        }
    }

    /// ApiErrorCodeのテスト
    mod api_error_code_tests {
        use super::*;

        #[test]
        fn test_api_error_code_as_str() {
            assert_eq!(ApiErrorCode::InvalidInput.as_str(), "INVALID_INPUT");
            assert_eq!(ApiErrorCode::FileTooLarge.as_str(), "FILE_TOO_LARGE");
            assert_eq!(ApiErrorCode::UnsupportedFormat.as_str(), "UNSUPPORTED_FORMAT");
            assert_eq!(ApiErrorCode::ProcessingFailed.as_str(), "PROCESSING_FAILED");
            assert_eq!(ApiErrorCode::ModelNotLoaded.as_str(), "MODEL_NOT_LOADED");
            assert_eq!(ApiErrorCode::ServerOverloaded.as_str(), "SERVER_OVERLOADED");
            assert_eq!(ApiErrorCode::InternalError.as_str(), "INTERNAL_ERROR");
        }

        #[test]
        fn test_api_error_code_clone() {
            let error = ApiErrorCode::ProcessingFailed;
            let cloned = error.clone();

            assert_eq!(error.as_str(), cloned.as_str());
        }
    }

    /// APIリクエスト/レスポンスモデルのテスト
    mod api_models_tests {
        use super::*;

        #[test]
        fn test_transcribe_request_default_values() {
            let request = TranscribeRequest {
                language: None,
                translate_to_english: None,
                include_timestamps: None,
            };

            assert!(request.language.is_none());
            assert!(request.translate_to_english.is_none());
            assert!(request.include_timestamps.is_none());
        }

        #[test]
        fn test_transcribe_request_with_values() {
            let request = TranscribeRequest {
                language: Some("ja".to_string()),
                translate_to_english: Some(true),
                include_timestamps: Some(false),
            };

            assert_eq!(request.language, Some("ja".to_string()));
            assert_eq!(request.translate_to_english, Some(true));
            assert_eq!(request.include_timestamps, Some(false));
        }

        #[test]
        fn test_transcribe_response() {
            let segments = vec![
                TranscriptionSegment::new("Hello".to_string(), 0, 1000),
                TranscriptionSegment::new("World".to_string(), 1000, 2000),
            ];

            let response = TranscribeResponse {
                text: "Hello World".to_string(),
                language: Some("en".to_string()),
                duration_ms: Some(2000),
                segments: Some(segments.clone()),
                processing_time_ms: 1500,
            };

            assert_eq!(response.text, "Hello World");
            assert_eq!(response.language, Some("en".to_string()));
            assert_eq!(response.duration_ms, Some(2000));
            assert_eq!(response.processing_time_ms, 1500);
            assert!(response.segments.is_some());
            assert_eq!(response.segments.unwrap().len(), 2);
        }

        #[test]
        fn test_model_info() {
            let model_info = ModelInfo {
                name: "Test Model".to_string(),
                file_path: "/path/to/model.bin".to_string(),
                size_mb: 500,
                description: "Test description".to_string(),
                language_support: vec!["en".to_string(), "ja".to_string()],
                is_available: true,
            };

            assert_eq!(model_info.name, "Test Model");
            assert_eq!(model_info.file_path, "/path/to/model.bin");
            assert_eq!(model_info.size_mb, 500);
            assert_eq!(model_info.description, "Test description");
            assert_eq!(model_info.language_support.len(), 2);
            assert!(model_info.is_available);
        }

        #[test]
        fn test_models_response() {
            let models = vec![
                ModelInfo {
                    name: "Model 1".to_string(),
                    file_path: "/path/1.bin".to_string(),
                    size_mb: 100,
                    description: "Description 1".to_string(),
                    language_support: vec!["en".to_string()],
                    is_available: true,
                },
                ModelInfo {
                    name: "Model 2".to_string(),
                    file_path: "/path/2.bin".to_string(),
                    size_mb: 200,
                    description: "Description 2".to_string(),
                    language_support: vec!["ja".to_string()],
                    is_available: false,
                },
            ];

            let response = ModelsResponse {
                models: models.clone(),
                current_model: "Model 1".to_string(),
            };

            assert_eq!(response.models.len(), 2);
            assert_eq!(response.current_model, "Model 1");
        }

        #[test]
        fn test_health_response() {
            let health = HealthResponse {
                status: "healthy".to_string(),
                version: "1.0.0".to_string(),
                model_loaded: true,
                uptime_seconds: 3600,
                memory_usage_mb: Some(512),
            };

            assert_eq!(health.status, "healthy");
            assert_eq!(health.version, "1.0.0");
            assert!(health.model_loaded);
            assert_eq!(health.uptime_seconds, 3600);
            assert_eq!(health.memory_usage_mb, Some(512));
        }

        #[test]
        fn test_error_response() {
            let error = ErrorResponse {
                error: "Something went wrong".to_string(),
                code: "INTERNAL_ERROR".to_string(),
                details: Some("Stack trace here".to_string()),
            };

            assert_eq!(error.error, "Something went wrong");
            assert_eq!(error.code, "INTERNAL_ERROR");
            assert_eq!(error.details, Some("Stack trace here".to_string()));
        }
    }

    /// シリアライゼーション/デシリアライゼーションのテスト
    mod serialization_tests {
        use super::*;

        #[test]
        fn test_transcription_segment_json_serialization() {
            let segment = TranscriptionSegment::new(
                "Test text".to_string(),
                1000,
                2000,
            );

            let json = serde_json::to_string(&segment).unwrap();
            let deserialized: TranscriptionSegment = serde_json::from_str(&json).unwrap();

            assert_eq!(segment.text, deserialized.text);
            assert_eq!(segment.start_time_ms, deserialized.start_time_ms);
            assert_eq!(segment.end_time_ms, deserialized.end_time_ms);
        }

        #[test]
        fn test_server_stats_json_serialization() {
            let mut stats = ServerStats::default();
            stats.record_request();
            stats.record_success(1500);

            let json = serde_json::to_string(&stats).unwrap();
            let deserialized: ServerStats = serde_json::from_str(&json).unwrap();

            assert_eq!(stats.total_requests, deserialized.total_requests);
            assert_eq!(stats.successful_transcriptions, deserialized.successful_transcriptions);
            assert_eq!(stats.average_processing_time_ms, deserialized.average_processing_time_ms);
        }

        #[test]
        fn test_transcribe_request_json_serialization() {
            let request = TranscribeRequest {
                language: Some("ja".to_string()),
                translate_to_english: Some(false),
                include_timestamps: Some(true),
            };

            let json = serde_json::to_string(&request).unwrap();
            let deserialized: TranscribeRequest = serde_json::from_str(&json).unwrap();

            assert_eq!(request.language, deserialized.language);
            assert_eq!(request.translate_to_english, deserialized.translate_to_english);
            assert_eq!(request.include_timestamps, deserialized.include_timestamps);
        }

        #[test]
        fn test_model_catalog_json_serialization() {
            let catalog = ModelCatalog::default();

            let json = serde_json::to_string(&catalog).unwrap();
            let deserialized: ModelCatalog = serde_json::from_str(&json).unwrap();

            assert_eq!(catalog.models.len(), deserialized.models.len());

            for key in catalog.models.keys() {
                assert!(deserialized.models.contains_key(key));
            }
        }
    }
}
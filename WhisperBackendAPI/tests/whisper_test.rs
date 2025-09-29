use std::fs;
use tempfile::TempDir;
use WhisperBackendAPI::{config::Config, whisper::*};

#[cfg(test)]
mod whisper_tests {
    use super::*;

    /// テスト用設定を作成
    fn create_test_config(temp_dir: &TempDir) -> Config {
        let mut config = Config::default();

        // テスト用のディレクトリを設定
        let models_dir = temp_dir.path().join("models");
        fs::create_dir_all(&models_dir).unwrap();

        // ダミーのモデルファイルを作成
        let model_file = models_dir.join("test_model.bin");
        fs::write(&model_file, b"dummy whisper model data").unwrap();

        config.whisper.model_path = model_file.to_string_lossy().to_string();
        config.whisper.enable_gpu = false; // テスト環境ではCPUを使用
        config.whisper.language = "auto".to_string();
        config.performance.whisper_threads = 4;
        config.paths.models_dir = models_dir.to_string_lossy().to_string();

        config
    }

    /// テスト用音声データを生成（サイン波）
    fn generate_test_audio_samples(sample_rate: u32, duration_seconds: f32) -> Vec<f32> {
        let total_samples = (sample_rate as f32 * duration_seconds) as usize;
        let frequency = 440.0; // A4音
        let mut samples = Vec::with_capacity(total_samples);

        for i in 0..total_samples {
            let t = i as f32 / sample_rate as f32;
            let sample = (2.0 * std::f32::consts::PI * frequency * t).sin() * 0.5;
            samples.push(sample);
        }

        samples
    }

    /// WhisperEngineの初期化テスト（モデルファイルなし）
    #[test]
    fn test_whisper_engine_new_model_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = Config::default();
        config.whisper.model_path = "/nonexistent/model.bin".to_string();

        let result = WhisperEngine::new(&config.whisper.model_path, &config);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Whisperモデルファイルが見つかりません"));
    }

    /// WhisperEngineの初期化テスト（正常系）
    #[test]
    #[ignore] // 実際のwhisper-rsライブラリが必要なため通常テストでは無視
    fn test_whisper_engine_new_success() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);

        let result = WhisperEngine::new(&config.whisper.model_path, &config);
        // 実際のモデルファイルではないため失敗するが、パス検証は通る
        assert!(result.is_err()); // ダミーファイルなので失敗するが、これは期待される動作
    }

    /// get_supported_languagesのテスト
    #[test]
    fn test_get_supported_languages() {
        let languages = get_supported_languages();

        assert!(!languages.is_empty());
        assert!(languages.contains(&"auto"));
        assert!(languages.contains(&"en"));
        assert!(languages.contains(&"ja"));
        assert!(languages.contains(&"zh"));
        assert!(languages.contains(&"de"));
        assert!(languages.contains(&"fr"));

        // 特定の言語数をチェック（将来的に変更される可能性があるため範囲チェック）
        assert!(languages.len() > 50);
        assert!(languages.len() < 200);
    }

    /// get_language_nameのテスト
    #[test]
    fn test_get_language_name() {
        assert_eq!(get_language_name("en"), "English");
        assert_eq!(get_language_name("ja"), "Japanese");
        assert_eq!(get_language_name("zh"), "Chinese");
        assert_eq!(get_language_name("de"), "German");
        assert_eq!(get_language_name("es"), "Spanish");
        assert_eq!(get_language_name("ru"), "Russian");
        assert_eq!(get_language_name("ko"), "Korean");
        assert_eq!(get_language_name("fr"), "French");
        assert_eq!(get_language_name("pt"), "Portuguese");
        assert_eq!(get_language_name("tr"), "Turkish");
        assert_eq!(get_language_name("pl"), "Polish");
        assert_eq!(get_language_name("ca"), "Catalan");
        assert_eq!(get_language_name("nl"), "Dutch");
        assert_eq!(get_language_name("ar"), "Arabic");
        assert_eq!(get_language_name("sv"), "Swedish");
        assert_eq!(get_language_name("it"), "Italian");
        assert_eq!(get_language_name("auto"), "Auto Detect");
        assert_eq!(get_language_name("unknown_language"), "Unknown");
    }

    /// preprocess_audioのテスト
    #[test]
    fn test_preprocess_audio() {
        let mut samples = vec![0.1, -0.2, 0.3, -0.4, 2.0, -1.5]; // 2.0, -1.5は範囲外

        preprocess_audio(&mut samples);

        // 正規化により全てのサンプルが±0.95以内に収まることを確認
        for sample in &samples {
            assert!(*sample >= -0.95 && *sample <= 0.95);
        }

        // 最大絶対値が0.95に近いことを確認（正規化されている）
        let max_abs = samples.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);
        assert!(max_abs > 0.9 && max_abs <= 0.95);
    }

    /// normalize_audio（preprocess_audio内部関数）のテスト
    #[test]
    fn test_normalize_audio() {
        let mut samples = vec![1.0, -2.0, 0.5, -1.5];

        preprocess_audio(&mut samples); // normalize_audioを含む

        // 正規化により最大絶対値が0.95になることを確認
        let max_abs = samples.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);
        assert!((max_abs - 0.95).abs() < 0.001); // 浮動小数点の誤差を考慮
    }

    /// normalize_audio - 空の配列のテスト
    #[test]
    fn test_normalize_audio_empty() {
        let mut samples: Vec<f32> = vec![];

        preprocess_audio(&mut samples);

        assert!(samples.is_empty()); // 変化なし
    }

    /// normalize_audio - 全てゼロの配列のテスト
    #[test]
    fn test_normalize_audio_all_zeros() {
        let mut samples = vec![0.0, 0.0, 0.0, 0.0];

        preprocess_audio(&mut samples);

        // ゼロのままであることを確認
        for sample in &samples {
            assert_eq!(*sample, 0.0);
        }
    }

    /// normalize_audio - 既に正規化済みの配列のテスト
    #[test]
    fn test_normalize_audio_already_normalized() {
        let mut samples = vec![0.5, -0.3, 0.2, -0.1];
        let original = samples.clone();

        preprocess_audio(&mut samples);

        // 既に範囲内なので大きく変化しないことを確認
        for (i, &sample) in samples.iter().enumerate() {
            let ratio = sample / original[i];
            assert!(ratio > 0.9 && ratio < 1.1); // 10%以内の変化
        }
    }

    /// ModelInfoのテスト
    #[test]
    fn test_model_info() {
        let model_info = ModelInfo {
            is_loaded: true,
            language: Some("ja".to_string()),
            threads: 8,
            enable_gpu: false,
        };

        assert!(model_info.is_loaded);
        assert_eq!(model_info.language, Some("ja".to_string()));
        assert_eq!(model_info.threads, 8);
        assert!(!model_info.enable_gpu);
    }

    /// TranscriptionResultのテスト
    #[test]
    fn test_transcription_result() {
        use WhisperBackendAPI::models::TranscriptionSegment;

        let segments = vec![
            TranscriptionSegment::new("Hello".to_string(), 0, 1000),
            TranscriptionSegment::new("World".to_string(), 1000, 2000),
        ];

        let result = TranscriptionResult {
            text: "Hello World".to_string(),
            segments: segments.clone(),
            language: Some("en".to_string()),
            processing_time_ms: 1500,
        };

        assert_eq!(result.text, "Hello World");
        assert_eq!(result.segments.len(), 2);
        assert_eq!(result.language, Some("en".to_string()));
        assert_eq!(result.processing_time_ms, 1500);
    }

    /// WhisperEnginePoolの基本テスト
    #[test]
    #[ignore] // 実際のwhisper-rsライブラリが必要なため通常テストでは無視
    fn test_whisper_engine_pool_new() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);

        // プールサイズ3でテスト
        let result = WhisperEnginePool::new(&config.whisper.model_path, &config, 3);
        // ダミーモデルファイルなので失敗するが、これは期待される動作
        assert!(result.is_err());
    }

    /// 音声処理のエラーハンドリングテスト
    mod error_handling_tests {
        use super::*;

        #[test]
        fn test_empty_audio_samples() {
            let empty_samples: Vec<f32> = vec![];
            let mut samples = empty_samples;

            // 空のサンプル配列でも正規化は安全に実行される
            preprocess_audio(&mut samples);
            assert!(samples.is_empty());
        }

        #[test]
        fn test_single_sample() {
            let mut samples = vec![1.5];

            preprocess_audio(&mut samples);

            // 単一サンプルでも正規化される
            assert_eq!(samples.len(), 1);
            assert!((samples[0].abs() - 0.95).abs() < 0.001);
        }

        #[test]
        fn test_extreme_values() {
            let mut samples = vec![100000.0, -50000.0, 0.001];

            preprocess_audio(&mut samples);

            // 極値でも正常に正規化される
            for sample in &samples {
                assert!(*sample >= -0.95 && *sample <= 0.95);
            }

            let max_abs = samples.iter().map(|&x| x.abs()).fold(0.0f32, f32::max);
            assert!((max_abs - 0.95).abs() < 0.01);
        }
    }

    /// 言語サポートのテスト
    mod language_support_tests {
        use super::*;

        #[test]
        fn test_all_supported_languages_have_names() {
            let languages = get_supported_languages();

            for &lang in languages.iter() {
                let name = get_language_name(lang);
                assert_ne!(name, ""); // 空文字列ではない
                assert_ne!(name, "Unknown"); // "Unknown"は未知言語用
            }
        }

        #[test]
        fn test_language_name_coverage() {
            // 主要言語のカバレッジをテスト
            let major_languages = vec![
                ("en", "English"),
                ("ja", "Japanese"),
                ("zh", "Chinese"),
                ("de", "German"),
                ("es", "Spanish"),
                ("fr", "French"),
                ("ru", "Russian"),
                ("ko", "Korean"),
                ("pt", "Portuguese"),
                ("it", "Italian"),
                ("ar", "Arabic"),
            ];

            for (code, expected_name) in major_languages {
                assert!(get_supported_languages().contains(&code));
                assert_eq!(get_language_name(code), expected_name);
            }
        }

        #[test]
        fn test_auto_language_detection() {
            assert!(get_supported_languages().contains(&"auto"));
            assert_eq!(get_language_name("auto"), "Auto Detect");
        }
    }

    /// 統計テスト（性能関連）
    mod performance_tests {
        use super::*;

        #[test]
        fn test_audio_sample_generation() {
            let samples = generate_test_audio_samples(16000, 1.0);

            assert_eq!(samples.len(), 16000); // 1秒 × 16kHz = 16000サンプル

            // サイン波なので値が-1から1の範囲内
            for &sample in samples.iter() {
                assert!(sample >= -1.0 && sample <= 1.0);
            }

            // サイン波の特性：最大値と最小値が存在することを確認
            let max_val = samples.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
            let min_val = samples.iter().cloned().fold(f32::INFINITY, f32::min);

            assert!(max_val > 0.3); // サイン波の振幅
            assert!(min_val < -0.3);
        }

        #[test]
        fn test_audio_sample_generation_different_durations() {
            let durations = vec![0.1, 0.5, 2.0, 5.0];
            let sample_rate = 44100;

            for duration in durations {
                let samples = generate_test_audio_samples(sample_rate, duration);
                let expected_length = (sample_rate as f32 * duration) as usize;

                assert_eq!(samples.len(), expected_length);
                assert!(!samples.is_empty());
            }
        }

        #[test]
        fn test_normalize_performance() {
            // 大きなサンプル配列での性能テスト
            let mut large_samples: Vec<f32> = (0..100000)
                .map(|i| (i as f32 / 1000.0).sin() * 2.0) // 範囲外の値を含むサイン波
                .collect();

            let start = std::time::Instant::now();
            preprocess_audio(&mut large_samples);
            let duration = start.elapsed();

            // 正規化が完了していることを確認
            let max_abs = large_samples
                .iter()
                .map(|&x| x.abs())
                .fold(0.0f32, f32::max);
            assert!(max_abs <= 0.95);

            // 処理時間が合理的であることを確認（1秒以内）
            assert!(duration.as_secs() < 1);
        }
    }

    /// クローンとコピーのテスト
    mod clone_tests {
        use super::*;

        #[test]
        fn test_model_info_clone() {
            let original = ModelInfo {
                is_loaded: true,
                language: Some("en".to_string()),
                threads: 4,
                enable_gpu: true,
            };

            let cloned = original.clone();

            assert_eq!(original.is_loaded, cloned.is_loaded);
            assert_eq!(original.language, cloned.language);
            assert_eq!(original.threads, cloned.threads);
            assert_eq!(original.enable_gpu, cloned.enable_gpu);
        }

        #[test]
        fn test_transcription_result_clone() {
            use WhisperBackendAPI::models::TranscriptionSegment;

            let segments = vec![TranscriptionSegment::new("Test".to_string(), 0, 1000)];

            let original = TranscriptionResult {
                text: "Test".to_string(),
                segments: segments.clone(),
                language: Some("en".to_string()),
                processing_time_ms: 500,
            };

            let cloned = original.clone();

            assert_eq!(original.text, cloned.text);
            assert_eq!(original.segments.len(), cloned.segments.len());
            assert_eq!(original.language, cloned.language);
            assert_eq!(original.processing_time_ms, cloned.processing_time_ms);
        }
    }
}

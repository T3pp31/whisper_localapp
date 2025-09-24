use std::fs;
use std::io::Write;
use tempfile::TempDir;
use WhisperBackendAPI::{audio::*, config::Config};

#[cfg(test)]
mod audio_tests {
    use super::*;

    /// テスト用のWAVファイルデータを生成（44バイトヘッダー + 16-bit PCM）
    fn create_test_wav_data(sample_rate: u32, duration_seconds: f32) -> Vec<u8> {
        let samples_per_channel = (sample_rate as f32 * duration_seconds) as usize;
        let data_size = samples_per_channel * 2; // 16-bit = 2 bytes per sample
        let file_size = 36 + data_size;

        let mut wav_data = Vec::new();

        // WAVヘッダー
        wav_data.extend_from_slice(b"RIFF");
        wav_data.extend_from_slice(&(file_size as u32).to_le_bytes());
        wav_data.extend_from_slice(b"WAVE");
        wav_data.extend_from_slice(b"fmt ");
        wav_data.extend_from_slice(&16u32.to_le_bytes()); // fmt chunk size
        wav_data.extend_from_slice(&1u16.to_le_bytes());  // PCM format
        wav_data.extend_from_slice(&1u16.to_le_bytes());  // mono
        wav_data.extend_from_slice(&sample_rate.to_le_bytes());
        wav_data.extend_from_slice(&(sample_rate * 2).to_le_bytes()); // byte rate
        wav_data.extend_from_slice(&2u16.to_le_bytes());  // block align
        wav_data.extend_from_slice(&16u16.to_le_bytes()); // bits per sample
        wav_data.extend_from_slice(b"data");
        wav_data.extend_from_slice(&(data_size as u32).to_le_bytes());

        // サイン波データ（440Hz A4音）
        for i in 0..samples_per_channel {
            let t = i as f32 / sample_rate as f32;
            let sample = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 16383.0;
            wav_data.extend_from_slice(&(sample as i16).to_le_bytes());
        }

        wav_data
    }

    /// テスト用設定を作成
    fn create_test_config(temp_dir: &TempDir) -> Config {
        let mut config = Config::default();
        config.paths.temp_dir = temp_dir.path().to_string_lossy().to_string();
        config.limits.max_file_size_mb = 10;
        config.limits.max_audio_duration_minutes = 5;
        config.audio.sample_rate = 16000;
        config
    }

    /// AudioProcessorの初期化テスト
    #[test]
    fn test_audio_processor_new() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);

        let result = AudioProcessor::new(&config);
        assert!(result.is_ok());
    }

    /// detect_audio_formatのテスト
    #[test]
    fn test_detect_audio_format_by_extension() {
        let temp_dir = TempDir::new().unwrap();

        // 拡張子による判定テスト
        let test_files = vec![
            ("test.wav", "wav"),
            ("test.mp3", "mp3"),
            ("test.m4a", "m4a"),
            ("test.flac", "flac"),
            ("test.ogg", "ogg"),
        ];

        for (filename, expected) in test_files {
            let file_path = temp_dir.path().join(filename);
            fs::write(&file_path, b"dummy").unwrap();

            let result = detect_audio_format(&file_path).unwrap();
            assert_eq!(result, expected);
        }
    }

    /// detect_audio_formatのマジックナンバーテスト
    #[test]
    fn test_detect_audio_format_by_magic_number() {
        let temp_dir = TempDir::new().unwrap();

        // WAVファイルのマジックナンバーテスト
        let wav_file = temp_dir.path().join("test_no_ext");
        let wav_header = b"RIFF\x24\x08\x00\x00WAVEfmt ";
        fs::write(&wav_file, wav_header).unwrap();

        let result = detect_audio_format(&wav_file).unwrap();
        assert_eq!(result, "wav");

        // MP3ファイルのマジックナンバーテスト
        let mp3_file = temp_dir.path().join("test_no_ext2");
        let mp3_header = b"ID3\x03\x00\x00\x00\x00\x00\x00";
        fs::write(&mp3_file, mp3_header).unwrap();

        let result = detect_audio_format(&mp3_file).unwrap();
        assert_eq!(result, "mp3");

        // FLACファイルのマジックナンバーテスト
        let flac_file = temp_dir.path().join("test_no_ext3");
        let flac_header = b"fLaC\x00\x00\x00\x22";
        fs::write(&flac_file, flac_header).unwrap();

        let result = detect_audio_format(&flac_file).unwrap();
        assert_eq!(result, "flac");
    }

    /// format_file_sizeのテスト
    #[test]
    fn test_format_file_size() {
        assert_eq!(format_file_size(500), "500 B");
        assert_eq!(format_file_size(1024), "1.0 KB");
        assert_eq!(format_file_size(1536), "1.5 KB");
        assert_eq!(format_file_size(1024 * 1024), "1.0 MB");
        assert_eq!(format_file_size(1024 * 1024 * 1024), "1.0 GB");
        assert_eq!(format_file_size(1536 * 1024 * 1024), "1.5 GB");
    }

    /// AudioMetadataの基本テスト
    #[test]
    fn test_audio_metadata() {
        let metadata = AudioMetadata {
            duration_seconds: 120.5,
            sample_rate: 44100,
            channels: 2,
            file_size_bytes: 1024000,
            format: "wav".to_string(),
        };

        assert_eq!(metadata.duration_seconds, 120.5);
        assert_eq!(metadata.sample_rate, 44100);
        assert_eq!(metadata.channels, 2);
        assert_eq!(metadata.file_size_bytes, 1024000);
        assert_eq!(metadata.format, "wav");
    }

    /// ProcessedAudioの基本テスト
    #[test]
    fn test_processed_audio() {
        let samples = vec![0.1, 0.2, -0.1, -0.2];
        let metadata = AudioMetadata {
            duration_seconds: 1.0,
            sample_rate: 44100,
            channels: 1,
            file_size_bytes: 1000,
            format: "wav".to_string(),
        };

        let processed = ProcessedAudio {
            samples: samples.clone(),
            sample_rate: 16000,
            duration_ms: 250,
            original_metadata: metadata,
        };

        assert_eq!(processed.samples, samples);
        assert_eq!(processed.sample_rate, 16000);
        assert_eq!(processed.duration_ms, 250);
        assert_eq!(processed.original_metadata.duration_seconds, 1.0);
    }

    /// is_supported_formatのテスト
    #[test]
    fn test_is_supported_format() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);
        let processor = AudioProcessor::new(&config).unwrap();

        // サポート対象の形式
        assert!(processor.is_supported_format("test.wav"));
        assert!(processor.is_supported_format("test.mp3"));
        assert!(processor.is_supported_format("test.m4a"));
        assert!(processor.is_supported_format("test.flac"));
        assert!(processor.is_supported_format("test.ogg"));

        // 大文字小文字の違い
        assert!(processor.is_supported_format("TEST.WAV"));
        assert!(processor.is_supported_format("Test.Mp3"));

        // mp4の特殊処理（m4aがサポートされていればmp4もサポート）
        assert!(processor.is_supported_format("test.mp4"));

        // サポート対象外の形式
        assert!(!processor.is_supported_format("test.txt"));
        assert!(!processor.is_supported_format("test.pdf"));
        assert!(!processor.is_supported_format("test"));
    }

    /// validate_file_sizeのテスト
    #[test]
    fn test_validate_file_size() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);
        let processor = AudioProcessor::new(&config).unwrap();

        // 制限内のファイルサイズ（10MB以下）
        let result = processor.validate_file_size(5 * 1024 * 1024);
        assert!(result.is_ok());

        let result = processor.validate_file_size(10 * 1024 * 1024);
        assert!(result.is_ok());

        // 制限を超えるファイルサイズ
        let result = processor.validate_file_size(11 * 1024 * 1024);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("ファイルサイズが制限を超えています"));
    }

    /// validate_audio_durationのテスト
    #[test]
    fn test_validate_audio_duration() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);
        let processor = AudioProcessor::new(&config).unwrap();

        // 制限内の音声長（5分以下）
        let metadata = AudioMetadata {
            duration_seconds: 240.0, // 4分
            sample_rate: 16000,
            channels: 1,
            file_size_bytes: 1000,
            format: "wav".to_string(),
        };
        let result = processor.validate_audio_duration(&metadata);
        assert!(result.is_ok());

        let metadata = AudioMetadata {
            duration_seconds: 300.0, // 5分ちょうど
            sample_rate: 16000,
            channels: 1,
            file_size_bytes: 1000,
            format: "wav".to_string(),
        };
        let result = processor.validate_audio_duration(&metadata);
        assert!(result.is_ok());

        // 制限を超える音声長
        let metadata = AudioMetadata {
            duration_seconds: 360.0, // 6分
            sample_rate: 16000,
            channels: 1,
            file_size_bytes: 1000,
            format: "wav".to_string(),
        };
        let result = processor.validate_audio_duration(&metadata);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("音声ファイルが長すぎます"));
    }

    /// create_temp_file_from_bytesのテスト
    #[test]
    fn test_create_temp_file_from_bytes() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);
        let processor = AudioProcessor::new(&config).unwrap();

        let test_data = b"test audio data";
        let filename = "test.wav";

        let temp_file = processor.create_temp_file_from_bytes(test_data, filename).unwrap();

        // ファイルが作成されることを確認
        assert!(temp_file.path().exists());

        // ファイル内容が正しいことを確認
        let content = fs::read(temp_file.path()).unwrap();
        assert_eq!(content, test_data);

        // 拡張子が適切であることを確認
        assert!(temp_file.path().to_string_lossy().ends_with(".wav"));
    }

    /// probe_metadataのテスト（実際のWAVファイル）
    #[test]
    fn test_probe_metadata_wav() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);
        let processor = AudioProcessor::new(&config).unwrap();

        // 1秒のWAVファイルを作成
        let wav_data = create_test_wav_data(44100, 1.0);
        let wav_file = temp_dir.path().join("test.wav");
        fs::write(&wav_file, wav_data).unwrap();

        let metadata = processor.probe_metadata(&wav_file).unwrap();

        assert_eq!(metadata.sample_rate, 44100);
        assert_eq!(metadata.channels, 1);
        assert_eq!(metadata.format, "wav");
        assert!(metadata.duration_seconds > 0.9 && metadata.duration_seconds < 1.1); // 約1秒
        assert!(metadata.file_size_bytes > 0);
    }

    /// probe_metadata - ファイルが存在しない場合
    #[test]
    fn test_probe_metadata_file_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);
        let processor = AudioProcessor::new(&config).unwrap();

        let nonexistent_file = temp_dir.path().join("nonexistent.wav");
        let result = processor.probe_metadata(&nonexistent_file);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("音声ファイルが見つかりません"));
    }

    /// load_audio_file - 基本テスト
    #[test]
    fn test_load_audio_file() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = create_test_config(&temp_dir);
        config.audio.sample_rate = 44100; // リサンプリングを避けるため元と同じSRに設定
        let mut processor = AudioProcessor::new(&config).unwrap();

        // 0.5秒のWAVファイルを作成
        let wav_data = create_test_wav_data(44100, 0.5);
        let wav_file = temp_dir.path().join("test.wav");
        fs::write(&wav_file, wav_data).unwrap();

        let samples = processor.load_audio_file(&wav_file).unwrap();

        assert!(!samples.is_empty());
        // サンプル数が期待値に近いことを確認（0.5秒 × 44100Hz ≈ 22050サンプル）
        assert!(samples.len() > 20000 && samples.len() < 25000);

        // サンプルがf32範囲内であることを確認
        for sample in &samples {
            assert!(*sample >= -1.0 && *sample <= 1.0);
        }
    }

    /// load_audio_file - ファイルが存在しない場合
    #[test]
    fn test_load_audio_file_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);
        let mut processor = AudioProcessor::new(&config).unwrap();

        let nonexistent_file = temp_dir.path().join("nonexistent.wav");
        let result = processor.load_audio_file(&nonexistent_file);

        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("音声ファイルが見つかりません"));
    }

    /// process_audio_from_bytes - 統合テスト
    #[test]
    fn test_process_audio_from_bytes() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);
        let mut processor = AudioProcessor::new(&config).unwrap();

        // 1秒のWAVファイルデータを作成
        let wav_data = create_test_wav_data(44100, 1.0);
        let filename = "test.wav";

        let processed = processor.process_audio_from_bytes(&wav_data, filename).unwrap();

        assert!(!processed.samples.is_empty());
        assert_eq!(processed.sample_rate, 16000); // 設定されたターゲットSR
        assert!(processed.duration_ms > 950 && processed.duration_ms < 1050); // 約1秒
        assert_eq!(processed.original_metadata.sample_rate, 44100);
        assert_eq!(processed.original_metadata.format, "wav");
    }

    /// process_audio_file - 統合テスト
    #[test]
    fn test_process_audio_file() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);
        let mut processor = AudioProcessor::new(&config).unwrap();

        // 2秒のWAVファイルを作成
        let wav_data = create_test_wav_data(44100, 2.0);
        let wav_file = temp_dir.path().join("test.wav");
        fs::write(&wav_file, wav_data).unwrap();

        let processed = processor.process_audio_file(&wav_file).unwrap();

        assert!(!processed.samples.is_empty());
        assert_eq!(processed.sample_rate, 16000);
        assert!(processed.duration_ms > 1950 && processed.duration_ms < 2050); // 約2秒
        assert_eq!(processed.original_metadata.sample_rate, 44100);
        assert_eq!(processed.original_metadata.format, "wav");
    }

    /// 異なるサンプリングレートでのリサンプリングテスト
    #[test]
    fn test_resampling() {
        let temp_dir = TempDir::new().unwrap();
        let mut config = create_test_config(&temp_dir);
        config.audio.sample_rate = 8000; // 低いSRに設定してリサンプリングを強制
        let mut processor = AudioProcessor::new(&config).unwrap();

        // 44.1kHzのWAVファイルを作成
        let wav_data = create_test_wav_data(44100, 1.0);
        let wav_file = temp_dir.path().join("test.wav");
        fs::write(&wav_file, wav_data).unwrap();

        let processed = processor.process_audio_file(&wav_file).unwrap();

        assert_eq!(processed.sample_rate, 8000); // リサンプリング後のSR
        assert_eq!(processed.original_metadata.sample_rate, 44100); // 元のSR
        // リサンプリング後のサンプル数が適切であることを確認
        assert!(processed.samples.len() > 7500 && processed.samples.len() < 8500); // 約8000サンプル
    }

    /// 空のファイルに対するエラーハンドリング
    #[test]
    fn test_empty_file_error() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);
        let processor = AudioProcessor::new(&config).unwrap();

        // 空のファイルを作成
        let empty_file = temp_dir.path().join("empty.wav");
        fs::write(&empty_file, b"").unwrap();

        let result = processor.probe_metadata(&empty_file);
        assert!(result.is_err());
    }

    /// 不正な音声ファイルに対するエラーハンドリング
    #[test]
    fn test_invalid_audio_file_error() {
        let temp_dir = TempDir::new().unwrap();
        let config = create_test_config(&temp_dir);
        let mut processor = AudioProcessor::new(&config).unwrap();

        // テキストファイルを音声ファイルとして処理しようとする
        let text_file = temp_dir.path().join("invalid.wav");
        fs::write(&text_file, b"This is not an audio file").unwrap();

        let result = processor.load_audio_file(&text_file);
        assert!(result.is_err());
    }
}
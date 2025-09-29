use std::time::Instant;
use WhisperBackendAPI::{config::Config, whisper::WhisperEngine};

/// GPU使用テストの統合テスト
#[cfg(test)]
mod gpu_tests {
    use super::*;

    /// GPU初期化テスト
    #[test]
    fn test_gpu_initialization() {
        let config = get_test_config();

        println!("=== GPU初期化テスト ===");
        println!("設定でのGPU有効化: {}", config.whisper.enable_gpu);

        match WhisperEngine::new(&config.whisper.model_path, &config) {
            Ok(engine) => {
                let model_info = engine.get_model_info();
                println!("✓ Whisperエンジンの初期化に成功");
                println!("実際のGPU状態: {}", model_info.enable_gpu);
                assert!(model_info.is_loaded, "モデルが正しくロードされていません");
            }
            Err(e) => {
                println!("⚠ Whisperエンジンの初期化に失敗: {}", e);
                // テストでは失敗してもパニックしない（モデルファイルがない場合など）
            }
        }
    }

    /// 環境変数確認テスト
    #[test]
    fn test_environment_variables() {
        println!("=== 環境変数確認テスト ===");

        let whisper_cublas = std::env::var("WHISPER_CUBLAS").unwrap_or_default();
        let whisper_opencl = std::env::var("WHISPER_OPENCL").unwrap_or_default();
        let cuda_path = std::env::var("CUDA_PATH").ok();

        println!("WHISPER_CUBLAS: {}", whisper_cublas);
        println!("WHISPER_OPENCL: {}", whisper_opencl);
        println!("CUDA_PATH: {:?}", cuda_path);

        // フィーチャーフラグの確認
        #[cfg(feature = "cuda")]
        {
            println!("✓ CUDA feature is enabled");
        }
        #[cfg(not(feature = "cuda"))]
        {
            println!("- CUDA feature is disabled");
        }

        #[cfg(feature = "opencl")]
        {
            println!("✓ OpenCL feature is enabled");
        }
        #[cfg(not(feature = "opencl"))]
        {
            println!("- OpenCL feature is disabled");
        }

        // GPU有効化の条件チェック
        if whisper_cublas == "1" {
            println!("✓ CUBLAS GPU acceleration should be enabled");
        } else {
            println!("- CUBLAS GPU acceleration is not set");
        }
    }

    /// GPU vs CPU 性能比較テスト（実際のモデルが必要）
    #[test]
    #[ignore] // 通常のテスト実行では無視（手動実行用）
    fn test_gpu_vs_cpu_performance() {
        println!("=== GPU vs CPU 性能比較テスト ===");

        // テスト用音声データ（1秒のサイン波）
        let sample_rate = 16000;
        let duration_seconds = 1;
        let samples = generate_test_audio(sample_rate, duration_seconds);

        // CPU設定でテスト
        let mut cpu_config = get_test_config();
        cpu_config.whisper.enable_gpu = false;

        if let Ok(cpu_engine) = WhisperEngine::new(&cpu_config.whisper.model_path, &cpu_config) {
            let start_time = Instant::now();
            match cpu_engine.transcribe(&samples) {
                Ok(_text) => {
                    let cpu_time = start_time.elapsed();
                    println!("CPU処理時間: {:.2}ms", cpu_time.as_secs_f64() * 1000.0);
                }
                Err(e) => println!("CPU処理エラー: {}", e),
            }
        }

        // GPU設定でテスト
        let mut gpu_config = get_test_config();
        gpu_config.whisper.enable_gpu = true;

        if let Ok(gpu_engine) = WhisperEngine::new(&gpu_config.whisper.model_path, &gpu_config) {
            let start_time = Instant::now();
            match gpu_engine.transcribe(&samples) {
                Ok(_text) => {
                    let gpu_time = start_time.elapsed();
                    println!("GPU処理時間: {:.2}ms", gpu_time.as_secs_f64() * 1000.0);
                }
                Err(e) => println!("GPU処理エラー: {}", e),
            }
        }
    }

    /// CUDA/OpenCLライブラリの検出テスト
    #[test]
    fn test_gpu_library_detection() {
        println!("=== GPUライブラリ検出テスト ===");

        // CUDA関連ライブラリの確認
        let cuda_paths = vec![
            "/usr/local/cuda/lib64/libcudart.so",
            "/usr/lib/x86_64-linux-gnu/libcudart.so",
            "/usr/local/cuda/lib64/libcublas.so",
            "/usr/lib/x86_64-linux-gnu/libcublas.so",
        ];

        for path in cuda_paths {
            if std::path::Path::new(path).exists() {
                println!("✓ Found: {}", path);
            } else {
                println!("- Not found: {}", path);
            }
        }

        // OpenCLライブラリの確認
        let opencl_paths = vec![
            "/usr/lib/x86_64-linux-gnu/libOpenCL.so",
            "/usr/local/lib/libOpenCL.so",
        ];

        for path in opencl_paths {
            if std::path::Path::new(path).exists() {
                println!("✓ Found: {}", path);
            } else {
                println!("- Not found: {}", path);
            }
        }

        // nvidia-smiの確認
        match std::process::Command::new("nvidia-smi")
            .arg("--version")
            .output()
        {
            Ok(output) => {
                if output.status.success() {
                    println!("✓ nvidia-smi is available");
                    let version = String::from_utf8_lossy(&output.stdout);
                    println!("  Version info: {}", version.lines().next().unwrap_or(""));
                } else {
                    println!("- nvidia-smi command failed");
                }
            }
            Err(_) => {
                println!("- nvidia-smi not found");
            }
        }
    }

    /// テスト用設定を取得
    pub(crate) fn get_test_config() -> Config {
        let mut config = Config::default();
        // テスト用のモデルパス（実際のファイルが存在する場合のみ動作）
        config.whisper.model_path = "models/ggml-base.bin".to_string();
        config.whisper.enable_gpu = true;
        config
    }

    /// テスト用音声データを生成（サイン波）
    pub(crate) fn generate_test_audio(sample_rate: u32, duration_seconds: u32) -> Vec<f32> {
        let total_samples = sample_rate * duration_seconds;
        let frequency = 440.0; // A4音
        let mut samples = Vec::with_capacity(total_samples as usize);

        for i in 0..total_samples {
            let t = i as f32 / sample_rate as f32;
            let sample = (2.0 * std::f32::consts::PI * frequency * t).sin() * 0.5;
            samples.push(sample);
        }

        samples
    }
}

/// ベンチマークテスト用（cargo bench で実行）
#[cfg(test)]
mod bench_tests {
    use super::*;

    /// GPU文字起こしのベンチマーク
    #[test]
    #[ignore]
    fn bench_gpu_transcription() {
        let config = get_test_config();
        if let Ok(engine) = WhisperEngine::new(&config.whisper.model_path, &config) {
            let samples = generate_test_audio(16000, 5); // 5秒の音声

            let iterations = 10;
            let mut total_time = std::time::Duration::new(0, 0);

            println!("GPU文字起こしベンチマーク（{}回実行）", iterations);

            for i in 0..iterations {
                let start = Instant::now();
                match engine.transcribe(&samples) {
                    Ok(_) => {
                        let elapsed = start.elapsed();
                        total_time += elapsed;
                        println!("実行 {}: {:.2}ms", i + 1, elapsed.as_secs_f64() * 1000.0);
                    }
                    Err(e) => {
                        println!("エラー: {}", e);
                        return;
                    }
                }
            }

            let avg_time = total_time / iterations;
            println!("平均処理時間: {:.2}ms", avg_time.as_secs_f64() * 1000.0);
        }
    }

    use super::gpu_tests::{generate_test_audio, get_test_config};
}

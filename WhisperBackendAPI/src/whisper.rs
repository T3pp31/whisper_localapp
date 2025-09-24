use crate::config::Config;
use crate::models::TranscriptionSegment;
use anyhow::Result;
use std::path::Path;
use std::sync::Arc;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// Whisperエンジンのラッパー（スレッドセーフ）
/// - whisper-rs の `WhisperContext` を `Arc` で共有
/// - 各推論は独立した `state` を生成して実行する安全な使い方
pub struct WhisperEngine {
    context: Arc<WhisperContext>,
    language: Option<String>,
    whisper_threads: i32,
    enable_gpu: bool,
}

/// Whisper処理の結果
#[derive(Debug, Clone)]
pub struct TranscriptionResult {
    pub text: String,
    pub segments: Vec<TranscriptionSegment>,
    pub language: Option<String>,
    pub processing_time_ms: u64,
}

impl WhisperEngine {
    /// 新しいWhisperEngineを作成
    /// - モデルファイルの存在確認 → WhisperContext 初期化
    /// - Config からスレッド数/言語/GPU 設定を反映
    pub fn new(model_path: &str, config: &Config) -> Result<Self> {
        // モデルファイルの存在確認
        if !Path::new(model_path).exists() {
            return Err(anyhow::anyhow!(
                "Whisperモデルファイルが見つかりません: {}\n\
                 以下のコマンドでモデルをダウンロードしてください:\n\
                 wget https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin -P models/",
                model_path
            ));
        }

        // Whisperコンテキストの初期化
        let mut ctx_params = WhisperContextParameters::default();

        // GPU使用の設定（whisper-rs/whisper.cpp 側が対応していれば有効化）
        // - 実際にGPUコードが使われるかはビルド時のバックエンド有効化に依存します
        //   例: CUDA (cuBLAS) を使う場合は `WHISPER_CUBLAS=1` 等のフラグでビルド
        ctx_params.use_gpu = config.whisper.enable_gpu;

        // GPU設定のデバッグ情報
        println!("=== GPU設定情報 ===");
        println!("設定でGPU有効化: {}", config.whisper.enable_gpu);
        println!("WhisperContextParameters.use_gpu: {}", ctx_params.use_gpu);

        // 環境変数の確認
        if let Ok(cublas) = std::env::var("WHISPER_CUBLAS") {
            println!("WHISPER_CUBLAS環境変数: {}", cublas);
        } else {
            println!("WHISPER_CUBLAS環境変数: 未設定");
        }

        if let Ok(opencl) = std::env::var("WHISPER_OPENCL") {
            println!("WHISPER_OPENCL環境変数: {}", opencl);
        } else {
            println!("WHISPER_OPENCL環境変数: 未設定");
        }

        // CUDA情報の確認
        #[cfg(feature = "cuda")]
        {
            println!("CUDA feature is enabled");
        }
        #[cfg(not(feature = "cuda"))]
        {
            println!("CUDA feature is disabled");
        }

        #[cfg(feature = "opencl")]
        {
            println!("OpenCL feature is enabled");
        }
        #[cfg(not(feature = "opencl"))]
        {
            println!("OpenCL feature is disabled");
        }

        // コンテキスト作成（GPU有効時に失敗した場合はCPUでフォールバック）
        let (context, gpu_actually_enabled) = match WhisperContext::new_with_params(model_path, ctx_params) {
            Ok(ctx) => {
                if config.whisper.enable_gpu {
                    println!("✓ GPU対応のWhisperコンテキストの初期化に成功しました");
                    println!("✓ GPUアクセラレーションが有効です");
                } else {
                    println!("✓ CPU専用のWhisperコンテキストの初期化に成功しました");
                }
                (ctx, config.whisper.enable_gpu)
            },
            Err(e) => {
                if config.whisper.enable_gpu {
                    eprintln!(
                        "⚠ GPU初期化に失敗しました。CPUで再試行します: {}",
                        e
                    );
                    let mut cpu_params = WhisperContextParameters::default();
                    cpu_params.use_gpu = false;
                    let cpu_context = WhisperContext::new_with_params(model_path, cpu_params)
                        .map_err(|e| anyhow::anyhow!("Whisperコンテキストの初期化に失敗: {}", e))?;
                    println!("✓ CPUでのWhisperコンテキスト初期化にフォールバックしました");
                    (cpu_context, false)
                } else {
                    return Err(anyhow::anyhow!(
                        "Whisperコンテキストの初期化に失敗: {}",
                        e
                    ));
                }
            }
        };

        let language = match config.whisper.language.trim() {
            "" => None,
            lang if lang.eq_ignore_ascii_case("auto") => None,
            lang => Some(lang.to_string()),
        };

        println!(
            "✓ Whisperモデルを読み込みました: {} (GPU: {} -> 実際: {})",
            model_path,
            if config.whisper.enable_gpu { "設定有効" } else { "設定無効" },
            if gpu_actually_enabled { "有効" } else { "無効" }
        );
        println!("==================");

        Ok(Self {
            context: Arc::new(context),
            language,
            whisper_threads: config.performance.whisper_threads as i32,
            enable_gpu: gpu_actually_enabled,
        })
    }

    /// 基本的な文字起こし（タイムスタンプなし）
    /// - `transcribe_internal` を include_timestamps=false で呼び出し、テキストのみ返す
    pub fn transcribe(&self, audio_data: &[f32]) -> Result<String> {
        let start_time = std::time::Instant::now();

        let result = self.transcribe_internal(audio_data, None, false, false)?;

        let processing_time = start_time.elapsed().as_millis() as u64;
        println!("文字起こし完了: {}ms", processing_time);

        Ok(result.text)
    }

    /// タイムスタンプ付きの詳細な文字起こし
    /// - セグメントの開始/終了時刻（10ms 単位）をミリ秒に変換して返す
    pub fn transcribe_with_timestamps(
        &self,
        audio_data: &[f32],
        translate_to_english: bool,
        language: Option<&str>,
    ) -> Result<TranscriptionResult> {
        let start_time = std::time::Instant::now();

        let result = self.transcribe_internal(audio_data, language, translate_to_english, true)?;

        let processing_time_ms = start_time.elapsed().as_millis() as u64;

        println!(
            "詳細文字起こし完了: {}ms, {}セグメント",
            processing_time_ms,
            result.segments.len()
        );

        Ok(TranscriptionResult {
            text: result.text,
            segments: result.segments,
            language: result.language,
            processing_time_ms,
        })
    }

    /// 内部的な文字起こし処理
    /// - whisper-rs の `state.full` を用いる標準フロー
    /// - language 指定（上書き）/翻訳モード/タイムスタンプ出力を切り替え
    fn transcribe_internal(
        &self,
        audio_data: &[f32],
        language_override: Option<&str>,
        translate_to_english: bool,
        include_timestamps: bool,
    ) -> Result<TranscriptionResult> {
        // 音声データの検証
        if audio_data.is_empty() {
            return Err(anyhow::anyhow!("音声データが空です"));
        }

        // Whisperの状態を作成（各リクエストごとに新しい状態）
        let mut state = self
            .context
            .create_state()
            .map_err(|e| anyhow::anyhow!("Whisper状態の作成に失敗: {}", e))?;

        // パラメータを設定
        // - 言語/スレッド数/翻訳/タイムスタンプ等
        let params = self.make_params(language_override, translate_to_english, include_timestamps);

        // 文字起こし実行
        if self.enable_gpu {
            println!("🚀 GPU使用で文字起こしを開始します...");
        } else {
            println!("🖥️  CPU使用で文字起こしを開始します...");
        }

        let transcribe_start = std::time::Instant::now();
        state
            .full(params, audio_data)
            .map_err(|e| anyhow::anyhow!("文字起こしに失敗: {}", e))?;

        let transcribe_duration = transcribe_start.elapsed();
        println!(
            "⏱️  推論処理時間: {:.2}ms ({})",
            transcribe_duration.as_secs_f64() * 1000.0,
            if self.enable_gpu { "GPU" } else { "CPU" }
        );

        // 結果の取得
        // - セグメントごとにテキスト/開始(t0)/終了(t1) を参照
        let segment_count = state
            .full_n_segments()
            .map_err(|e| anyhow::anyhow!("セグメント数の取得に失敗: {}", e))?;

        let mut text_parts = Vec::new();
        let mut segments = Vec::new();

        for i in 0..segment_count {
            let segment_text = state
                .full_get_segment_text(i)
                .map_err(|e| anyhow::anyhow!("セグメント{}のテキスト取得に失敗: {}", i, e))?;

            let segment_text = segment_text.trim().to_string();
            text_parts.push(segment_text.clone());

            if include_timestamps {
                let start_time = state
                    .full_get_segment_t0(i)
                    .map_err(|e| anyhow::anyhow!("セグメント{}の開始時間取得に失敗: {}", i, e))?;

                let end_time = state
                    .full_get_segment_t1(i)
                    .map_err(|e| anyhow::anyhow!("セグメント{}の終了時間取得に失敗: {}", i, e))?;

                segments.push(TranscriptionSegment {
                    text: segment_text,
                    start_time_ms: start_time as u64 * 10, // centisecondsをミリ秒に変換
                    end_time_ms: end_time as u64 * 10,
                });
            }
        }

        // 全体のテキストを結合
        let full_text = text_parts.join("").trim().to_string();

        let final_text = if full_text.is_empty() {
            "(音声を認識できませんでした)".to_string()
        } else {
            full_text
        };

        // 言語検出結果を取得（可能であれば）
        // - 明示指定が優先。無ければエンジン既定（Config）を返す
        let detected_language = language_override.map(|lang| lang.to_string())
            .or_else(|| self.language.clone());

        Ok(TranscriptionResult {
            text: final_text,
            segments,
            language: detected_language,
            processing_time_ms: 0, // 呼び出し元で設定
        })
    }

    /// Whisperパラメータを作成
    /// - Greedy デコード（best_of=1）
    /// - 進捗ログ等はサーバーコンソールを汚さないよう無効化
    fn make_params<'a>(
        &'a self,
        language_override: Option<&'a str>,
        translate_to_english: bool,
        include_timestamps: bool,
    ) -> FullParams<'a, 'static> {
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

        // 言語設定（優先順位: 呼び出し時の指定 > エンジン既定）
        if let Some(language) = language_override.or(self.language.as_deref()) {
            params.set_language(Some(language));
        }

        // スレッド数の設定
        params.set_n_threads(self.whisper_threads);

        // 出力制御
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(include_timestamps);

        // 翻訳モード
        params.set_translate(translate_to_english);

        // タイムスタンプの設定
        if include_timestamps {
            params.set_no_timestamps(false);
        }

        params
    }

    /// モデル情報を取得
    pub fn get_model_info(&self) -> ModelInfo {
        ModelInfo {
            is_loaded: true,
            language: self.language.clone(),
            threads: self.whisper_threads,
            enable_gpu: self.enable_gpu,
        }
    }

    /// WhisperContextへの参照を取得（スレッドセーフ）
    pub fn get_context(&self) -> Arc<WhisperContext> {
        Arc::clone(&self.context)
    }
}

// Implement Debug without requiring inner WhisperContext to be Debug
impl std::fmt::Debug for WhisperEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WhisperEngine")
            .field("language", &self.language)
            .field("whisper_threads", &self.whisper_threads)
            .field("enable_gpu", &self.enable_gpu)
            .finish()
    }
}

// スレッドセーフなクローンを実装
impl Clone for WhisperEngine {
    fn clone(&self) -> Self {
        Self {
            context: Arc::clone(&self.context),
            language: self.language.clone(),
            whisper_threads: self.whisper_threads,
            enable_gpu: self.enable_gpu,
        }
    }
}

/// モデル情報
#[derive(Debug, Clone, serde::Serialize)]
pub struct ModelInfo {
    pub is_loaded: bool,
    pub language: Option<String>,
    pub threads: i32,
    pub enable_gpu: bool,
}

/// Whisperエンジンプール（複数のリクエストを同時処理するため）
/// - 現状のコードでは未使用だが、将来的なスループット向上に備えた構造
pub struct WhisperEnginePool {
    engines: Vec<WhisperEngine>,
    current_index: std::sync::atomic::AtomicUsize,
}

impl WhisperEnginePool {
    /// 新しいエンジンプールを作成
    pub fn new(model_path: &str, config: &Config, pool_size: usize) -> Result<Self> {
        let mut engines = Vec::with_capacity(pool_size);

        for i in 0..pool_size {
            let engine = WhisperEngine::new(model_path, config)
                .map_err(|e| anyhow::anyhow!("エンジン{}の作成に失敗: {}", i, e))?;
            engines.push(engine);
        }

        println!("Whisperエンジンプールを作成しました: {}個のエンジン", pool_size);

        Ok(Self {
            engines,
            current_index: std::sync::atomic::AtomicUsize::new(0),
        })
    }

    /// 利用可能なエンジンを取得（ラウンドロビン方式）
    pub fn get_engine(&self) -> &WhisperEngine {
        let index = self.current_index.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        &self.engines[index % self.engines.len()]
    }

    /// プール内のエンジン数を取得
    pub fn size(&self) -> usize {
        self.engines.len()
    }
}

// =============================================================================
// Utility Functions
// =============================================================================

/// 音声データの前処理（ノイズ除去等）
/// - まずは振幅の基本正規化のみ。
/// - 追加のフィルタ処理は必要に応じて拡張可能。
pub fn preprocess_audio(audio_data: &mut [f32]) {
    // 基本的な正規化
    normalize_audio(audio_data);

    // 必要に応じて他の前処理を追加
    // - ハイパスフィルター
    // - ノイズゲート
    // - 自動ゲイン制御
}

/// 音声データの正規化
/// - 最大絶対値が0.95を超える場合のみスケーリング（増幅はしない）
fn normalize_audio(audio_data: &mut [f32]) {
    if audio_data.is_empty() {
        return;
    }

    // 最大絶対値を見つける
    let max_abs = audio_data
        .iter()
        .map(|&x| x.abs())
        .fold(0.0f32, f32::max);

    // すでに範囲内（<= 0.95）の場合は何もしない。
    if max_abs > 0.95 {
        let normalize_factor = 0.95 / max_abs;
        for sample in audio_data.iter_mut() {
            *sample *= normalize_factor;
        }
    }
}

/// サポートされている言語のリストを取得
pub fn get_supported_languages() -> Vec<&'static str> {
    vec![
        "auto", "en", "zh", "de", "es", "ru", "ko", "fr", "ja", "pt", "tr", "pl", "ca", "nl", "ar",
        "sv", "it", "id", "hi", "fi", "vi", "he", "uk", "el", "ms", "cs", "ro", "da", "hu", "ta",
        "no", "th", "ur", "hr", "bg", "lt", "la", "mi", "ml", "cy", "sk", "te", "fa", "lv", "bn",
        "sr", "az", "sl", "kn", "et", "mk", "br", "eu", "is", "hy", "ne", "mn", "bs", "kk", "sq",
        "sw", "gl", "mr", "pa", "si", "km", "sn", "yo", "so", "af", "oc", "ka", "be", "tg", "sd",
        "gu", "am", "yi", "lo", "uz", "fo", "ht", "ps", "tk", "nn", "mt", "sa", "lb", "my", "bo",
        "tl", "mg", "as", "tt", "haw", "ln", "ha", "ba", "jw", "su",
    ]
}

/// 言語コードから言語名を取得
pub fn get_language_name(code: &str) -> &'static str {
    match code {
        // Special
        "auto" => "Auto Detect",

        // Core languages
        "en" => "English",
        "zh" => "Chinese",
        "de" => "German",
        "es" => "Spanish",
        "ru" => "Russian",
        "ko" => "Korean",
        "fr" => "French",
        "ja" => "Japanese",
        "pt" => "Portuguese",
        "tr" => "Turkish",
        "pl" => "Polish",
        "ca" => "Catalan",
        "nl" => "Dutch",
        "ar" => "Arabic",
        "sv" => "Swedish",
        "it" => "Italian",

        // Extended set matching get_supported_languages()
        "id" => "Indonesian",
        "hi" => "Hindi",
        "fi" => "Finnish",
        "vi" => "Vietnamese",
        "he" => "Hebrew",
        "uk" => "Ukrainian",
        "el" => "Greek",
        "ms" => "Malay",
        "cs" => "Czech",
        "ro" => "Romanian",
        "da" => "Danish",
        "hu" => "Hungarian",
        "ta" => "Tamil",
        "no" => "Norwegian",
        "th" => "Thai",
        "ur" => "Urdu",
        "hr" => "Croatian",
        "bg" => "Bulgarian",
        "lt" => "Lithuanian",
        "la" => "Latin",
        "mi" => "Maori",
        "ml" => "Malayalam",
        "cy" => "Welsh",
        "sk" => "Slovak",
        "te" => "Telugu",
        "fa" => "Persian",
        "lv" => "Latvian",
        "bn" => "Bengali",
        "sr" => "Serbian",
        "az" => "Azerbaijani",
        "sl" => "Slovenian",
        "kn" => "Kannada",
        "et" => "Estonian",
        "mk" => "Macedonian",
        "br" => "Breton",
        "eu" => "Basque",
        "is" => "Icelandic",
        "hy" => "Armenian",
        "ne" => "Nepali",
        "mn" => "Mongolian",
        "bs" => "Bosnian",
        "kk" => "Kazakh",
        "sq" => "Albanian",
        "sw" => "Swahili",
        "gl" => "Galician",
        "mr" => "Marathi",
        "pa" => "Punjabi",
        "si" => "Sinhala",
        "km" => "Khmer",
        "sn" => "Shona",
        "yo" => "Yoruba",
        "so" => "Somali",
        "af" => "Afrikaans",
        "oc" => "Occitan",
        "ka" => "Georgian",
        "be" => "Belarusian",
        "tg" => "Tajik",
        "sd" => "Sindhi",
        "gu" => "Gujarati",
        "am" => "Amharic",
        "yi" => "Yiddish",
        "lo" => "Lao",
        "uz" => "Uzbek",
        "fo" => "Faroese",
        "ht" => "Haitian Creole",
        "ps" => "Pashto",
        "tk" => "Turkmen",
        "nn" => "Norwegian Nynorsk",
        "mt" => "Maltese",
        "sa" => "Sanskrit",
        "lb" => "Luxembourgish",
        "my" => "Burmese",
        "bo" => "Tibetan",
        "tl" => "Tagalog",
        "mg" => "Malagasy",
        "as" => "Assamese",
        "tt" => "Tatar",
        "haw" => "Hawaiian",
        "ln" => "Lingala",
        "ha" => "Hausa",
        "ba" => "Bashkir",
        "jw" => "Javanese",
        "su" => "Sundanese",

        _ => "Unknown",
    }
}

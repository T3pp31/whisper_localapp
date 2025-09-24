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

        // コンテキスト作成（GPU有効時に失敗した場合はCPUでフォールバック）
        let context = match WhisperContext::new_with_params(model_path, ctx_params) {
            Ok(ctx) => ctx,
            Err(e) => {
                if config.whisper.enable_gpu {
                    eprintln!(
                        "GPU初期化に失敗しました。CPUで再試行します: {}",
                        e
                    );
                    let mut cpu_params = WhisperContextParameters::default();
                    cpu_params.use_gpu = false;
                    WhisperContext::new_with_params(model_path, cpu_params)
                        .map_err(|e| anyhow::anyhow!("Whisperコンテキストの初期化に失敗: {}", e))?
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
            "Whisperモデルを読み込みました: {} (GPU: {})",
            model_path,
            if config.whisper.enable_gpu { "enabled" } else { "disabled" }
        );

        Ok(Self {
            context: Arc::new(context),
            language,
            whisper_threads: config.performance.whisper_threads as i32,
            enable_gpu: config.whisper.enable_gpu,
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
        state
            .full(params, audio_data)
            .map_err(|e| anyhow::anyhow!("文字起こしに失敗: {}", e))?;

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
#[derive(Debug, Clone)]
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
/// - 振幅の最大絶対値を 0.95 に収まるようスケーリング
fn normalize_audio(audio_data: &mut [f32]) {
    if audio_data.is_empty() {
        return;
    }

    // 最大絶対値を見つける
    let max_abs = audio_data
        .iter()
        .map(|&x| x.abs())
        .fold(0.0f32, f32::max);

    if max_abs > 0.0 {
        // 正規化係数を計算（最大値を0.95に制限）
        let normalize_factor = 0.95 / max_abs;

        // 正規化を適用
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
        "auto" => "Auto Detect",
        _ => "Unknown",
    }
}

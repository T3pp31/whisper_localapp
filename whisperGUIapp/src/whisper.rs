//! Whisper 本体（whisper-rs）との橋渡しを行うエンジン層。
//! - コンテキストの初期化・保持
//! - パラメータ組み立てと言語/翻訳などの制御
//! - プレーンテキスト or タイムスタンプ付きセグメントの取得

use crate::config::Config;
use anyhow::Result;
use std::path::Path;
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

/// whisper-rs を安全に扱うための薄いラッパー。
pub struct WhisperEngine {
    context: WhisperContext,
    language: Option<String>,
    whisper_threads: i32,
}

impl WhisperEngine {
    /// スレッド数を動的に更新する。
    pub fn set_threads(&mut self, n: usize) {
        let n = n.max(1) as i32;
        self.whisper_threads = n;
    }

    /// 実行ごとのパラメータを構築するヘルパ。
    fn make_params<'a>(&'a self, language_override: Option<&'a str>) -> FullParams<'a, 'static> {
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

        // 優先順位: 呼び出し時の指定 > エンジン既定
        if let Some(language) = language_override.or(self.language.as_deref()) {
            params.set_language(Some(language));
        }

        params.set_n_threads(self.whisper_threads);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);
        params.set_translate(false);

        params
    }

    /// モデルを読み込み、Whisper コンテキストを初期化する。
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

        // GPU設定の適用
        if config.whisper.use_gpu {
            ctx_params.use_gpu(true);
            eprintln!("GPU加速を有効化しました");
        } else {
            eprintln!("CPU推論を使用します");
        }

        let context = WhisperContext::new_with_params(model_path, ctx_params)
            .map_err(|e| anyhow::anyhow!("Whisperコンテキストの初期化に失敗: {}", e))?;

        let language = match config.whisper.language.trim() {
            "" => None,
            lang if lang.eq_ignore_ascii_case("auto") => None,
            lang => Some(lang.to_string()),
        };

        Ok(Self {
            context,
            language,
            whisper_threads: config.performance.whisper_threads as i32,
        })
    }

    /// プレーンテキストの文字起こしを実行する（タイムスタンプなし）。
    pub fn transcribe(&self, audio_data: &[f32]) -> Result<String> {
        // Whisperの状態を作成
        let mut state = self
            .context
            .create_state()
            .map_err(|e| anyhow::anyhow!("Whisper状態の作成に失敗: {}", e))?;

        // 音声データの検証
        if audio_data.is_empty() {
            return Err(anyhow::anyhow!("音声データが空です"));
        }

        // 文字起こし実行
        let params = self.make_params(None);
        state
            .full(params, audio_data)
            .map_err(|e| anyhow::anyhow!("文字起こしに失敗: {}", e))?;

        // 結果の取得（0.15系ではfull_n_segments()は直接i32を返す）
        let segment_count = state.full_n_segments();

        let mut result = String::new();

        for i in 0..segment_count {
            // 0.15系ではget_segment()を使用
            if let Some(segment) = state.get_segment(i) {
                let segment_text = segment
                    .to_str()
                    .map_err(|e| anyhow::anyhow!("セグメント{}のテキスト取得に失敗: {}", i, e))?;
                result.push_str(segment_text);
            }
        }

        // 結果の後処理
        let result = result.trim().to_string();

        if result.is_empty() {
            return Ok("(音声を認識できませんでした)".to_string());
        }

        Ok(result)
    }

    /// 言語をオーバーライドしてプレーンテキストの文字起こしを実行する。
    pub fn transcribe_with_language(&self, audio_data: &[f32], language: Option<&str>) -> Result<String> {
        let mut state = self
            .context
            .create_state()
            .map_err(|e| anyhow::anyhow!("Whisper状態の作成に失敗: {}", e))?;

        if audio_data.is_empty() {
            return Err(anyhow::anyhow!("音声データが空です"));
        }

        let params = self.make_params(language);
        state
            .full(params, audio_data)
            .map_err(|e| anyhow::anyhow!("文字起こしに失敗: {}", e))?;

        let segment_count = state.full_n_segments();

        let mut result = String::new();
        for i in 0..segment_count {
            if let Some(segment) = state.get_segment(i) {
                let segment_text = segment
                    .to_str()
                    .map_err(|e| anyhow::anyhow!("セグメント{}のテキスト取得に失敗: {}", i, e))?;
                result.push_str(segment_text);
            }
        }
        let result = result.trim().to_string();
        if result.is_empty() {
            return Ok("(音声を認識できませんでした)".to_string());
        }
        Ok(result)
    }

    /// タイムスタンプ付きで文字起こしを実行する。
    pub fn transcribe_with_timestamps(
        &self,
        audio_data: &[f32],
        translate_to_english: bool,
        language: Option<&str>,
    ) -> Result<Vec<TranscriptionSegment>> {
        let mut state = self
            .context
            .create_state()
            .map_err(|e| anyhow::anyhow!("Whisper状態の作成に失敗: {}", e))?;

        // タイムスタンプ付きのパラメータを設定
        let mut params = self.make_params(language);
        params.set_print_timestamps(true);
        params.set_translate(translate_to_english);

        state
            .full(params, audio_data)
            .map_err(|e| anyhow::anyhow!("文字起こしに失敗: {}", e))?;

        let segment_count = state.full_n_segments();

        let mut segments = Vec::new();

        for i in 0..segment_count {
            if let Some(segment) = state.get_segment(i) {
                let text = segment
                    .to_str()
                    .map_err(|e| anyhow::anyhow!("セグメント{}のテキスト取得に失敗: {}", i, e))?;

                // 0.15系ではstart_timestamp/end_timestamp()を使用（centiseconds = 10ms単位）
                let start_time = segment.start_timestamp();
                let end_time = segment.end_timestamp();

                segments.push(TranscriptionSegment {
                    text: text.trim().to_string(),
                    start_time_ms: start_time as u64 * 10, // centisecondsをミリ秒に変換
                    end_time_ms: end_time as u64 * 10,
                });
            }
        }

        Ok(segments)
    }
}

/// 1 セグメント分の認識結果。
#[derive(Debug, Clone)]
pub struct TranscriptionSegment {
    pub text: String,
    pub start_time_ms: u64,
    pub end_time_ms: u64,
}

impl TranscriptionSegment {
    /// SRT 1 エントリの文字列に整形する（index は 1 始まり）。
    pub fn to_srt_format(&self, index: usize) -> String {
        let start_time = Self::ms_to_srt_time(self.start_time_ms);
        let end_time = Self::ms_to_srt_time(self.end_time_ms);

        format!(
            "{}\n{} --> {}\n{}\n\n",
            index + 1,
            start_time,
            end_time,
            self.text
        )
    }

    /// ミリ秒を `HH:MM:SS,mmm` 形式へ変換。
    fn ms_to_srt_time(ms: u64) -> String {
        let total_seconds = ms / 1000;
        let milliseconds = ms % 1000;
        let seconds = total_seconds % 60;
        let minutes = (total_seconds / 60) % 60;
        let hours = total_seconds / 3600;

        format!(
            "{:02}:{:02}:{:02},{:03}",
            hours, minutes, seconds, milliseconds
        )
    }
}

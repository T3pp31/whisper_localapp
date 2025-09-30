use std::path::Path;

use crate::asr::AsrError;
use crate::config::WhisperModelConfig;

#[derive(Debug)]
pub struct WhisperEngine {
    ctx: whisper_rs::WhisperContext,
    cfg: WhisperModelConfig,
}

impl WhisperEngine {
    pub fn load(cfg: WhisperModelConfig) -> Result<Self, AsrError> {
        let path = Path::new(&cfg.model_path);
        let path_str = path.to_str().ok_or_else(|| AsrError::Processing { message: "invalid model path".to_string() })?;
        let ctx = whisper_rs::WhisperContext::new(path_str)
            .map_err(|e| AsrError::Processing { message: format!("failed to load whisper model: {e}") })?;
        Ok(Self { ctx, cfg })
    }

    /// i16 PCM（16kHz mono）を文字起こし
    pub fn transcribe_i16(&self, pcm_i16: &[i16], sample_rate_hz: i32, channels: i32) -> Result<String, AsrError> {
        if sample_rate_hz != 16000 || channels != 1 {
            return Err(AsrError::Processing { message: format!(
                "unsupported format: sample_rate={} channels={}", sample_rate_hz, channels
            )});
        }

        // i16 -> f32 へ正規化
        let mut pcm_f32 = vec![0.0f32; pcm_i16.len()];
        for (i, s) in pcm_i16.iter().enumerate() {
            pcm_f32[i] = (*s as f32) / 32768.0;
        }

        let mut state = self.ctx.create_state()
            .map_err(|e| AsrError::Processing { message: format!("failed to create whisper state: {e}") })?;

        let mut params = whisper_rs::FullParams::new(whisper_rs::SamplingStrategy::Greedy { best_of: 1 });
        params.set_n_threads(self.cfg.threads as i32);
        if self.cfg.language.to_ascii_lowercase() != "auto" {
            params.set_language(Some(&self.cfg.language));
        }
        params.set_translate(self.cfg.translate);

        state
            .full(params, &pcm_f32)
            .map_err(|e| AsrError::Processing { message: format!("whisper inference failed: {e}") })?;

        // セグメントを結合
        let num = state.full_n_segments().map_err(|e| AsrError::Processing { message: format!("segment count failed: {e}") })?;
        let mut text = String::new();
        for i in 0..num {
            let seg = state.full_get_segment_text(i).map_err(|e| AsrError::Processing { message: format!("get segment failed: {e}") })?;
            text.push_str(&seg);
        }
        Ok(text)
    }
}

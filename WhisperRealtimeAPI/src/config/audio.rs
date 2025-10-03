//! 音声処理に関する設定値
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct AudioProcessingConfig {
    pub input: InputFormat,
    pub target: TargetFormat,
    pub frame_assembler: FrameAssembler,
    pub normalization: NormalizationConfig,
}

impl AudioProcessingConfig {
    /// ターゲットの1フレームあたりサンプル数を計算
    pub fn target_frame_samples(&self) -> usize {
        (self.target.sample_rate_hz as f32 * self.frame_assembler.frame_duration_ms as f32 / 1000.0)
            as usize
    }

    /// 入力の1フレームあたりサンプル数を計算
    pub fn input_frame_samples(&self) -> usize {
        (self.input.sample_rate_hz as f32 * self.frame_assembler.frame_duration_ms as f32 / 1000.0)
            as usize
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct InputFormat {
    pub sample_rate_hz: u32,
    pub channels: u8,
    pub frame_ms: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TargetFormat {
    pub sample_rate_hz: u32,
    pub channels: u8,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FrameAssembler {
    pub frame_duration_ms: u32,
    pub jitter_buffer_ms: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct NormalizationConfig {
    pub target_rms_db: f32,
    pub limiter_threshold_db: f32,
    pub attack_ms: u32,
    pub release_ms: u32,
}

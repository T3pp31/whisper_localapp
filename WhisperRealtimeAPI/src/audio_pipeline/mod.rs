mod frame_reconstructor;
mod normalizer;
mod opus_decoder;
mod resampler;
mod utils;

use crate::config::AudioProcessingConfig;

use frame_reconstructor::FrameReconstructor;
use normalizer::LevelNormalizer;
use utils::interleaved_to_mono;

pub use opus_decoder::AudioOpusDecoder;
pub use resampler::LinearResampler;

#[derive(Debug)]
pub struct AudioPipeline {
    reconstructor: FrameReconstructor,
    resampler: LinearResampler,
    normalizer: LevelNormalizer,
    input_channels: u8,
}

impl AudioPipeline {
    pub fn new(config: AudioProcessingConfig) -> Self {
        let target_samples = config.target_frame_samples();
        Self {
            reconstructor: FrameReconstructor::new(target_samples),
            resampler: LinearResampler::new(
                config.input.sample_rate_hz,
                config.target.sample_rate_hz,
            ),
            normalizer: LevelNormalizer::new(
                config.normalization.target_rms_db,
                config.normalization.limiter_threshold_db,
            ),
            input_channels: config.input.channels,
        }
    }

    pub fn process(&mut self, frame: &[i16]) -> Vec<Vec<f32>> {
        let mono = interleaved_to_mono(frame, self.input_channels);
        let resampled = self.resampler.resample(&mono);
        let normalized = self.normalizer.normalize(&resampled);
        self.reconstructor.push(&normalized)
    }

    pub fn flush(&mut self) -> Option<Vec<f32>> {
        self.reconstructor.flush()
    }
}

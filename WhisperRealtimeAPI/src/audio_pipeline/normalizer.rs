#[derive(Debug, Clone)]
pub struct LevelNormalizer {
    target_rms: f32,
    limiter_threshold: f32,
}

impl LevelNormalizer {
    pub fn new(target_rms_db: f32, limiter_threshold_db: f32) -> Self {
        Self {
            target_rms: db_to_linear(target_rms_db),
            limiter_threshold: db_to_linear(limiter_threshold_db),
        }
    }

    pub fn normalize(&self, samples: &[f32]) -> Vec<f32> {
        if samples.is_empty() {
            return Vec::new();
        }

        let rms = root_mean_square(samples);
        if rms == 0.0 {
            return samples.to_vec();
        }

        let gain = self.target_rms / rms;
        samples
            .iter()
            .map(|sample| {
                let amplified = sample * gain;
                amplified.clamp(-self.limiter_threshold, self.limiter_threshold)
            })
            .collect()
    }
}

fn db_to_linear(db: f32) -> f32 {
    10_f32.powf(db / 20.0)
}

fn root_mean_square(samples: &[f32]) -> f32 {
    let sum = samples.iter().map(|s| s * s).sum::<f32>();
    (sum / samples.len() as f32).sqrt()
}

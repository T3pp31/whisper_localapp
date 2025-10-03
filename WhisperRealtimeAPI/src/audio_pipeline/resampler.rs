/// 単純な線形補間ベースのリサンプラ
#[derive(Debug, Clone)]
pub struct LinearResampler {
    input_rate: u32,
    output_rate: u32,
}

impl LinearResampler {
    /// 入出力サンプルレートを指定して作成
    pub fn new(input_rate: u32, output_rate: u32) -> Self {
        Self {
            input_rate,
            output_rate,
        }
    }

    /// 線形補間によりリサンプル
    pub fn resample(&self, samples: &[f32]) -> Vec<f32> {
        if self.input_rate == self.output_rate || samples.is_empty() {
            return samples.to_vec();
        }

        let ratio = self.output_rate as f64 / self.input_rate as f64;
        let output_len = (samples.len() as f64 * ratio).round() as usize;
        if output_len == 0 {
            return Vec::new();
        }

        let mut output = Vec::with_capacity(output_len);
        for n in 0..output_len {
            let position = n as f64 / ratio;
            let base_index = position.floor() as usize;
            let frac = position - base_index as f64;
            let a = samples
                .get(base_index)
                .copied()
                .unwrap_or(*samples.last().unwrap_or(&0.0));
            let b = samples
                .get(base_index + 1)
                .copied()
                .unwrap_or(*samples.last().unwrap_or(&a));
            let sample = a + (b - a) * frac as f32;
            output.push(sample);
        }
        output
    }
}

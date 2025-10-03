/// ターゲットサンプル数で区切られたフレーム列を生成するバッファ
#[derive(Debug)]
pub struct FrameReconstructor {
    target_samples: usize,
    buffer: Vec<f32>,
}

impl FrameReconstructor {
    /// 出力フレーム長（サンプル数）を指定して作成
    pub fn new(target_samples: usize) -> Self {
        Self {
            target_samples,
            buffer: Vec::with_capacity(target_samples * 2),
        }
    }

    /// 入力サンプルを追記し、満たした分だけフレームとして排出
    pub fn push(&mut self, frame: &[f32]) -> Vec<Vec<f32>> {
        self.buffer.extend_from_slice(frame);
        let mut frames = Vec::new();

        while self.buffer.len() >= self.target_samples {
            let remainder = self.buffer.split_off(self.target_samples);
            let produced = std::mem::replace(&mut self.buffer, remainder);
            frames.push(produced);
        }

        frames
    }

    /// 残りのサンプルを1フレームとして返す（残なしなら `None`）
    pub fn flush(&mut self) -> Option<Vec<f32>> {
        if self.buffer.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.buffer))
        }
    }
}

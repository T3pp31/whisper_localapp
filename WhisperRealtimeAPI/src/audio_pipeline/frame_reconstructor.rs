#[derive(Debug)]
pub struct FrameReconstructor {
    target_samples: usize,
    buffer: Vec<f32>,
}

impl FrameReconstructor {
    pub fn new(target_samples: usize) -> Self {
        Self {
            target_samples,
            buffer: Vec::with_capacity(target_samples * 2),
        }
    }

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

    pub fn flush(&mut self) -> Option<Vec<f32>> {
        if self.buffer.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.buffer))
        }
    }
}

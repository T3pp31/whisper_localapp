pub fn interleaved_to_mono(samples: &[i16], channels: u8) -> Vec<f32> {
    if channels == 0 {
        return Vec::new();
    }

    if channels == 1 {
        return samples
            .iter()
            .map(|s| *s as f32 / i16::MAX as f32)
            .collect();
    }

    let mut mono = Vec::with_capacity(samples.len() / channels as usize);
    for chunk in samples.chunks(channels as usize) {
        let sum: i32 = chunk.iter().map(|sample| *sample as i32).sum();
        let average = sum as f32 / channels as f32;
        mono.push(average / i16::MAX as f32);
    }
    mono
}

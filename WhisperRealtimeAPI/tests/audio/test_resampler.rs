use whisper_realtime_api::audio_pipeline::LinearResampler;

fn ramp(len: usize) -> Vec<f32> {
    (0..len).map(|i| i as f32 / len as f32).collect()
}

#[test]
fn downsampling_reduces_length_and_preserves_edges() {
    let resampler = LinearResampler::new(48_000, 16_000);
    let input = ramp(480);
    let output = resampler.resample(&input);

    assert!(output.len() < input.len());
    assert!(
        (output.first().copied().unwrap_or(0.0) - input.first().copied().unwrap()).abs() < 1e-6
    );
    assert!((output.last().copied().unwrap_or(0.0) - input.last().copied().unwrap()).abs() < 5e-2);
}

#[test]
fn upsampling_increases_length() {
    let resampler = LinearResampler::new(16_000, 48_000);
    let input = ramp(160);
    let output = resampler.resample(&input);

    assert!(output.len() > input.len());
}

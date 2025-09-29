use whisper_realtime_api::audio_pipeline::AudioPipeline;
use whisper_realtime_api::config::ConfigSet;

fn setup() -> (AudioPipeline, usize, usize) {
    let config = ConfigSet::load_from_dir("config").expect("config");
    let audio_cfg = config.audio.clone();
    let input_frame_samples = (audio_cfg.input.sample_rate_hz as usize
        * audio_cfg.frame_assembler.frame_duration_ms as usize)
        / 1000;
    let samples_per_frame = input_frame_samples * audio_cfg.input.channels as usize;
    let target_samples = audio_cfg.target_frame_samples();
    (
        AudioPipeline::new(audio_cfg),
        samples_per_frame,
        target_samples,
    )
}

#[test]
fn reconstructor_produces_consistent_frame_sizes() {
    let (mut pipeline, samples_per_frame, target_samples) = setup();

    let frame = vec![0_i16; samples_per_frame];
    let produced_first = pipeline.process(&frame);
    let produced_second = pipeline.process(&frame);

    for produced in produced_first.iter().chain(produced_second.iter()) {
        assert_eq!(produced.len(), target_samples);
    }

    if let Some(remainder) = pipeline.flush() {
        assert!(remainder.len() <= target_samples);
    }
}

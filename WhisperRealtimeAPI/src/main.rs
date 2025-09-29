use std::sync::Arc;

use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use whisper_realtime_api::asr::{AsrManager, MockAsrClient};
use whisper_realtime_api::audio_pipeline::AudioPipeline;
use whisper_realtime_api::config::ConfigSet;
use whisper_realtime_api::signaling::SignalingService;
use whisper_realtime_api::transport::{ConnectionProfile, InMemoryTransport, StreamKind};

fn main() {
    init_tracing();

    match ConfigSet::load_from_env() {
        Ok(config) => {
            let config = Arc::new(config);
            info!(root = ?config.root(), "configuration loaded");

            let signaling = SignalingService::with_default_validator(config.clone());
            let transport = InMemoryTransport::default();
            let asr_config = Arc::new(config.asr.clone());
            let asr_manager = AsrManager::new(MockAsrClient::new(asr_config.clone()), asr_config);
            let mut audio_pipeline = AudioPipeline::new(config.audio.clone());

            info!(
                max_sessions = config.system.resources.max_concurrent_sessions,
                "signaling service initialized"
            );

            let default_profile = ConnectionProfile::new(
                config.system.signaling.default_bitrate_kbps,
                120,
                [
                    StreamKind::Audio,
                    StreamKind::PartialTranscript,
                    StreamKind::FinalTranscript,
                    StreamKind::Control,
                ],
            );
            info!(
                bitrate = default_profile.max_bitrate_kbps,
                "default QUIC profile prepared"
            );

            let input_frame_samples = (config.audio.input.sample_rate_hz as usize
                * config.audio.frame_assembler.frame_duration_ms as usize)
                / 1000;
            let silent_frame =
                vec![0_i16; input_frame_samples * config.audio.input.channels as usize];
            let frames = audio_pipeline.process(&silent_frame);
            info!(
                produced_frames = frames.len(),
                "audio pipeline warm-up complete"
            );

            let _ = (signaling, transport, asr_manager);
        }
        Err(err) => {
            error!(error = ?err, "failed to load configuration");
            std::process::exit(1);
        }
    }
}

fn init_tracing() {
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .finish();

    if let Err(err) = tracing::subscriber::set_global_default(subscriber) {
        eprintln!("failed to install tracing subscriber: {err}");
    }
}

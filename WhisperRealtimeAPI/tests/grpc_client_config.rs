use std::sync::Arc;

use whisper_realtime_api::asr::GrpcAsrClient;
use whisper_realtime_api::config::ConfigSet;

#[test]
fn grpc_client_uses_audio_target_from_config() {
    let config = ConfigSet::load_from_dir("config").expect("load config");

    let asr_cfg = Arc::new(config.asr.clone());
    let endpoint = asr_cfg.service.endpoint.clone();

    let sample_rate = config.audio.target.sample_rate_hz as i32;
    let channels = config.audio.target.channels as i32;

    let client = GrpcAsrClient::new(endpoint, asr_cfg, sample_rate, channels);

    assert_eq!(client.sample_rate(), sample_rate);
    assert_eq!(client.channels(), channels);
}


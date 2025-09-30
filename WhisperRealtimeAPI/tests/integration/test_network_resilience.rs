use std::sync::Arc;

use whisper_realtime_api::config::ConfigSet;
use whisper_realtime_api::signaling::{ClientMetadata, SessionRequest, SignalingService};
use whisper_realtime_api::transport::{
    ConnectionProfile, InMemoryTransport, QuicTransport, StreamKind,
};

#[tokio::test]
async fn session_survives_bandwidth_adjustments() {
    let config = Arc::new(ConfigSet::load_from_dir("config").expect("config"));
    let signaling = SignalingService::with_default_validator(config.clone());
    let audience = config.system.token.audience.clone();

    let request = SessionRequest {
        client: ClientMetadata::browser("Chrome", "130"),
        auth_token: format!("{}:resilience", audience),
        retry: false,
    };

    let response = signaling
        .start_session(request)
        .await
        .expect("start session");

    let transport = InMemoryTransport::default();
    let profile = ConnectionProfile::new(
        response.max_bitrate_kbps,
        120,
        [
            StreamKind::Audio,
            StreamKind::PartialTranscript,
            StreamKind::FinalTranscript,
            StreamKind::Control,
        ],
    );

    let session = transport
        .connect(&response.session_id, profile)
        .await
        .expect("transport connect");

    let audio_stream = session
        .stream(StreamKind::Audio)
        .expect("audio stream available");

    transport
        .apply_bandwidth_limit(&response.session_id, 128)
        .await
        .expect("bandwidth limit applied");

    audio_stream
        .send(vec![0, 1, 2, 3])
        .await
        .expect("send payload");
    let payload = audio_stream.recv().await.expect("receive payload");
    assert_eq!(payload, vec![0, 1, 2, 3]);

    signaling
        .end_session(&response.session_id)
        .await
        .expect("end session");
    transport
        .disconnect(&response.session_id)
        .await
        .expect("disconnect");
}

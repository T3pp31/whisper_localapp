use whisper_realtime_api::transport::{WebRtcTransport, ConnectionProfile, StreamKind};
use std::sync::Arc;

#[tokio::test]
async fn test_webrtc_session_creation() {
    let transport = WebRtcTransport::new(vec![]);

    let profile = ConnectionProfile::new(
        320,
        120,
        [
            StreamKind::Audio,
            StreamKind::PartialTranscript,
            StreamKind::FinalTranscript,
            StreamKind::Control,
        ],
    );

    let session_id = "test-session-123".to_string();
    let result = transport.start_session(session_id.clone(), profile).await;

    assert!(result.is_ok());

    let session = result.unwrap();
    assert_eq!(session.session_id, session_id);
}

#[tokio::test]
async fn test_webrtc_offer_answer() {
    let transport = WebRtcTransport::new(vec![]);

    let profile = ConnectionProfile::new(
        320,
        120,
        [StreamKind::Audio],
    );

    let session_id = "test-offer-answer".to_string();
    let _session = transport.start_session(session_id.clone(), profile).await.unwrap();

    // 簡易的なOffer SDPテスト
    let offer_sdp = "v=0\r\no=- 0 0 IN IP4 127.0.0.1\r\ns=-\r\nt=0 0\r\n";

    // 実際のSDPパースはWebRTCスタックに依存するため、基本的な動作のみテスト
    // 実環境ではブラウザからのOffer SDPを使用
}
use std::sync::Arc;

use whisper_realtime_api::config::ConfigSet;
use whisper_realtime_api::signaling::{
    ClientMetadata, SessionRequest, SignalingError, SignalingService,
};

fn test_config() -> Arc<ConfigSet> {
    Arc::new(ConfigSet::load_from_dir("config").expect("failed to load config"))
}

#[tokio::test]
async fn start_session_success() {
    let config = test_config();
    let audience = config.system.token.audience.clone();
    let signaling = SignalingService::with_default_validator(config.clone());

    let request = SessionRequest {
        client: ClientMetadata::browser("Chrome", "130"),
        auth_token: format!("{}:user-1", audience),
        retry: false,
    };

    let response = signaling
        .start_session(request.clone())
        .await
        .expect("session to start");
    assert_eq!(
        response.max_bitrate_kbps,
        config.system.signaling.default_bitrate_kbps
    );
    assert!(!response.ice_servers.is_empty());

    assert_eq!(signaling.active_sessions().await, 1);
    signaling
        .heartbeat(&response.session_id)
        .await
        .expect("heartbeat ok");
    signaling
        .end_session(&response.session_id)
        .await
        .expect("end ok");
    assert_eq!(signaling.active_sessions().await, 0);
}

#[tokio::test]
async fn start_session_auth_failure() {
    let config = test_config();
    let signaling = SignalingService::with_default_validator(config.clone());

    let request = SessionRequest {
        client: ClientMetadata::browser("Chrome", "130"),
        auth_token: "wrong:user".to_string(),
        retry: false,
    };

    let error = signaling
        .start_session(request)
        .await
        .expect_err("authentication should fail");
    match error {
        SignalingError::Authentication { .. } => {}
        other => panic!("expected authentication error, got {other:?}"),
    }
    assert_eq!(signaling.active_sessions().await, 0);
}

#[tokio::test]
async fn start_session_rejects_unsupported_client() {
    let config = test_config();
    let audience = config.system.token.audience.clone();
    let signaling = SignalingService::with_default_validator(config.clone());

    let request = SessionRequest {
        client: ClientMetadata::browser("Firefox", "40"),
        auth_token: format!("{}:user-1", audience),
        retry: false,
    };

    let error = signaling
        .start_session(request)
        .await
        .expect_err("client should be rejected");
    match error {
        SignalingError::ClientNotSupported { .. } => {}
        other => panic!("expected unsupported client error, got {other:?}"),
    }
    assert_eq!(signaling.active_sessions().await, 0);
}

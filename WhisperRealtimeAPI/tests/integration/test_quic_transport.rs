use whisper_realtime_api::transport::{
    ConnectionProfile, InMemoryTransport, QuicTransport, StreamKind, TransportError,
};

#[tokio::test]
async fn connect_send_receive_disconnect() {
    let transport = InMemoryTransport::default();
    let profile = ConnectionProfile::new(320, 120, [StreamKind::Audio]);

    let session = transport.connect("sess-1", profile).await.expect("connect");

    let audio_stream = session
        .stream(StreamKind::Audio)
        .expect("audio stream available");

    audio_stream
        .send(vec![1_u8, 2, 3])
        .await
        .expect("send payload");
    let received = audio_stream.recv().await.expect("receive payload");
    assert_eq!(received, vec![1_u8, 2, 3]);

    transport.disconnect("sess-1").await.expect("disconnect");
}

#[tokio::test]
async fn prevent_duplicate_connections() {
    let transport = InMemoryTransport::default();
    let profile = ConnectionProfile::new(320, 120, [StreamKind::Audio]);
    transport
        .connect("dup", profile.clone())
        .await
        .expect("connect");

    let err = transport
        .connect("dup", profile)
        .await
        .expect_err("second connect should fail");
    match err {
        TransportError::AlreadyConnected { .. } => {}
        other => panic!("unexpected error {other:?}"),
    }
}

#[tokio::test]
async fn bandwidth_limit_is_applied() {
    let transport = InMemoryTransport::default();
    let profile = ConnectionProfile::new(320, 120, [StreamKind::Audio]);
    transport.connect("bw", profile).await.expect("connect");

    transport
        .apply_bandwidth_limit("bw", 128)
        .await
        .expect("apply limit");
    let current = transport.bandwidth_limit("bw").await.expect("get limit");
    assert_eq!(current, 128);
}

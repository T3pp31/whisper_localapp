use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;
use whisper_realtime_api::server::run_with_listener;
use whisper_realtime_api::signaling::websocket::{SignalingMessage, WebSocketSignalingHandler};

#[tokio::test]
#[ignore]
async fn websocket_server_sends_message_to_client() {
    let handler = WebSocketSignalingHandler::new();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let handler_for_server = handler.clone();

    tokio::spawn(async move {
        let _ = run_with_listener(listener, handler_for_server).await;
    });

    let (mut ws, _resp) = connect_async(format!("ws://{}/ws?session_id=s-1", addr))
        .await
        .expect("connect ok");

    handler
        .send_to_session(
            "s-1",
            SignalingMessage::Answer {
                session_id: "s-1".into(),
                sdp: "v=0".into(),
            },
        )
        .await
        .expect("send ok");

    if let Some(Ok(tokio_tungstenite::tungstenite::Message::Text(txt))) = ws.next().await {
        assert!(txt.contains("\"type\":\"answer\""));
    } else {
        panic!("expected text message");
    }
}

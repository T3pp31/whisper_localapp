use futures_util::StreamExt;
use tokio::net::TcpListener;
use tokio_tungstenite::connect_async;
use whisper_realtime_api::server::run_with_listener;
use whisper_realtime_api::signaling::websocket::WebSocketSignalingHandler;

// セッションIDが無い接続に対して、エラーメッセージが返ることを確認
// NOTE: サンドボックス環境ではソケット作成が制限され失敗するため既定で無効化
#[tokio::test]
#[ignore]
async fn websocket_missing_session_id_returns_error() {
    let handler = WebSocketSignalingHandler::new();
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();

    let h2 = handler.clone();
    tokio::spawn(async move {
        let _ = run_with_listener(listener, h2).await;
    });

    // session_id を付けずに接続
    let (mut ws, _resp) = connect_async(format!("ws://{}/ws", addr))
        .await
        .expect("connect ok");

    // 最初のメッセージはエラーであるはず
    if let Some(Ok(tokio_tungstenite::tungstenite::Message::Text(txt))) = ws.next().await {
        assert!(txt.contains("\"type\":\"error\""));
        assert!(txt.contains("missing session_id"));
    } else {
        panic!("expected error text message");
    }
}

use tokio_tungstenite::connect_async;
use futures_util::{SinkExt, StreamExt};
use serde_json::json;

/// WebSocketプロキシ機能の統合テスト
///
/// このテストはwhisperWEBuiとWhisperRealtimeAPIが両方起動している必要があります
#[tokio::test]
#[ignore] // デフォルトでは実行しない（手動実行用）
async fn test_websocket_proxy_connection() {
    // WebUIのWebSocketエンドポイントに接続
    let webui_ws_url = "ws://127.0.0.1:3001/ws/realtime/test-session-123";

    let (ws_stream, _) = connect_async(webui_ws_url)
        .await
        .expect("WebUI WebSocket接続に失敗");

    println!("WebUI WebSocket接続成功");

    let (mut write, mut read) = ws_stream.split();

    // Offerメッセージを送信
    let offer_message = json!({
        "type": "offer",
        "session_id": "test-session-123",
        "sdp": "v=0\r\no=- 0 0 IN IP4 127.0.0.1\r\ns=-\r\nt=0 0\r\n"
    });

    write
        .send(tokio_tungstenite::tungstenite::Message::Text(
            serde_json::to_string(&offer_message).unwrap(),
        ))
        .await
        .expect("Offerメッセージ送信に失敗");

    println!("Offerメッセージ送信完了");

    // Answerメッセージを受信（タイムアウト付き）
    let timeout_duration = std::time::Duration::from_secs(5);
    let response = tokio::time::timeout(timeout_duration, read.next()).await;

    match response {
        Ok(Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text)))) => {
            println!("受信メッセージ: {}", text);

            // メッセージの解析
            let parsed: serde_json::Value = serde_json::from_str(&text)
                .expect("JSON解析に失敗");

            // Answer or エラーメッセージを確認
            let message_type = parsed["type"].as_str().unwrap_or("");
            assert!(
                message_type == "answer" || message_type == "error",
                "予期しないメッセージタイプ: {}",
                message_type
            );

            println!("テスト成功: メッセージタイプ = {}", message_type);
        }
        Ok(Some(Ok(msg))) => {
            panic!("予期しないメッセージタイプを受信: {:?}", msg);
        }
        Ok(Some(Err(e))) => {
            panic!("WebSocketエラー: {}", e);
        }
        Ok(None) => {
            panic!("WebSocketが予期せず閉じられました");
        }
        Err(_) => {
            panic!("タイムアウト: バックエンドからの応答がありません");
        }
    }
}

/// 設定ファイルのテスト
#[test]
fn test_config_loading() {
    use whisper_webui::config::Config;

    let config = Config::load_or_create_default("config.toml")
        .expect("設定ファイルの読み込みに失敗");

    // リアルタイム設定が有効かチェック
    if config.realtime.enabled {
        assert!(
            !config.realtime.backend_ws_url.is_empty(),
            "backend_ws_urlが設定されていません"
        );
        assert!(
            config.realtime.connection_timeout_seconds > 0,
            "connection_timeout_secondsが無効です"
        );

        println!("リアルタイム設定:");
        println!("  enabled: {}", config.realtime.enabled);
        println!("  backend_ws_url: {}", config.realtime.backend_ws_url);
        println!(
            "  connection_timeout_seconds: {}",
            config.realtime.connection_timeout_seconds
        );
    } else {
        println!("リアルタイム機能は無効です");
    }
}
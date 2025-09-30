use std::net::SocketAddr;
use std::sync::Arc;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio_tungstenite::{accept_hdr_async, tungstenite::handshake::server::Request, WebSocketStream};
use tracing::{error, info, warn};

use crate::signaling::websocket::{SignalingMessage, WebSocketSignalingHandler};

#[derive(thiserror::Error, Debug)]
pub enum ServerError {
    #[error("bind error: {0}")]
    Bind(std::io::Error),
    #[error("accept error: {0}")]
    Accept(std::io::Error),
}

/// 指定アドレスにバインドしてWSサーバを起動
pub async fn bind_and_run(
    bind_addr: &str,
    handler: WebSocketSignalingHandler,
) -> Result<(), ServerError> {
    let listener = TcpListener::bind(bind_addr).await.map_err(ServerError::Bind)?;
    run_with_listener(listener, handler).await
}

/// 既存の`TcpListener`でWSサーバを起動（テストでも使用）
pub async fn run_with_listener(
    listener: TcpListener,
    handler: WebSocketSignalingHandler,
) -> Result<(), ServerError> {
    let local_addr = listener.local_addr().ok();
    if let Some(addr) = local_addr {
        info!(%addr, "WebSocket signaling server listening");
    }

    loop {
        let (stream, peer_addr) = match listener.accept().await {
            Ok(v) => v,
            Err(e) => return Err(ServerError::Accept(e)),
        };
        let handler = handler.clone();
        tokio::spawn(async move {
            if let Err(e) = handle_ws_connection(stream, handler, peer_addr).await {
                warn!(error = %e, "connection handling failed");
            }
        });
    }
}

async fn handle_ws_connection<S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static>(
    stream: S,
    handler: WebSocketSignalingHandler,
    peer: SocketAddr,
) -> Result<(), String> {
    // 接続時のHTTPリクエストを検査して session_id を取得
    let mut extracted_session_id: Option<String> = None;
    let ws = accept_hdr_async(stream, |req: &Request, mut resp| {
        let path = req.uri().path_and_query().map(|pq| pq.as_str()).unwrap_or("/");
        if let Some(id) = extract_session_id(path) {
            extracted_session_id = Some(id);
        }
        Ok(resp)
    })
    .await
    .map_err(|e| format!("websocket handshake failed: {e}"))?;

    let session_id = match extracted_session_id {
        Some(id) => id,
        None => {
            // セッションIDが無い場合、接続後にエラーを送出して終了
            let (mut tx, _rx) = ws.split();
            let err = SignalingMessage::Error {
                message: "missing session_id query parameter".to_string(),
            };
            let json = serde_json::to_string(&err).unwrap_or_else(|_| "{}".into());
            let _ = tx
                .send(tokio_tungstenite::tungstenite::Message::Text(json))
                .await;
            return Ok(());
        }
    };

    info!(%peer, %session_id, "accepted websocket connection");
    handler
        .handle_connection(ws, session_id)
        .await;
    Ok(())
}

fn extract_session_id(path_and_query: &str) -> Option<String> {
    // 例: "/ws?session_id=abc-123"
    let parts: Vec<&str> = path_and_query.split('?').collect();
    if parts.len() < 2 {
        return None;
    }
    let query = parts[1];
    for pair in query.split('&') {
        let mut kv = pair.splitn(2, '=');
        if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
            if k == "session_id" && !v.is_empty() {
                return Some(v.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures_util::{SinkExt, StreamExt};
    use tokio::net::TcpListener;
    use tokio_tungstenite::connect_async;

    #[tokio::test]
    async fn test_extract_session_id() {
        assert_eq!(extract_session_id("/ws?session_id=abc"), Some("abc".into()));
        assert_eq!(extract_session_id("/ws?x=1"), None);
        assert_eq!(extract_session_id("/ws"), None);
    }

    #[tokio::test]
    #[ignore]
    async fn test_ws_server_roundtrip() {
        let handler = WebSocketSignalingHandler::new();
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let h2 = handler.clone();
        tokio::spawn(async move {
            let _ = run_with_listener(listener, h2).await;
        });

        // 接続
        let url = format!("ws://{}/ws?session_id=test-1", addr);
        let (mut ws, _resp) = connect_async(url).await.expect("connect ok");

        // サーバー側からメッセージを送る
        handler
            .send_to_session(
                "test-1",
                SignalingMessage::Answer {
                    session_id: "test-1".to_string(),
                    sdp: "v=0".into(),
                },
            )
            .await
            .expect("send ok");

        // 受信確認
        if let Some(Ok(tokio_tungstenite::tungstenite::Message::Text(txt))) = ws.next().await {
            assert!(txt.contains("\"type\":\"answer\""));
        } else {
            panic!("expected text message");
        }
    }
}

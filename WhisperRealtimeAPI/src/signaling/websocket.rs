use futures_util::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::WebSocketStream;
use tracing::{debug, error, info, warn};

/// WebSocketシグナリングメッセージ
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SignalingMessage {
    /// Offer SDP
    #[serde(rename = "offer")]
    Offer { session_id: String, sdp: String },

    /// Answer SDP
    #[serde(rename = "answer")]
    Answer { session_id: String, sdp: String },

    /// ICE Candidate
    #[serde(rename = "ice_candidate")]
    IceCandidate {
        session_id: String,
        candidate: String,
    },

    /// エラー
    #[serde(rename = "error")]
    Error { message: String },
}

/// WebSocketシグナリングハンドラ
#[derive(Clone)]
pub struct WebSocketSignalingHandler {
    sessions: Arc<RwLock<HashMap<String, mpsc::Sender<SignalingMessage>>>>,
}

impl WebSocketSignalingHandler {
    pub fn new() -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// WebSocket接続を処理
    pub async fn handle_connection<S>(
        &self,
        ws_stream: WebSocketStream<S>,
        session_id: String,
    ) where
        S: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static,
    {
        info!(session_id = %session_id, "WebSocketシグナリング接続開始");

        let (mut ws_sender, mut ws_receiver) = ws_stream.split();
        let (tx, mut rx) = mpsc::channel::<SignalingMessage>(100);

        // セッション登録
        self.sessions.write().await.insert(session_id.clone(), tx.clone());

        // 送信タスク（サーバー→クライアント）
        let sessions_clone = self.sessions.clone();
        let session_id_for_send = session_id.clone();
        let send_task = tokio::spawn(async move {
            while let Some(message) = rx.recv().await {
                let json = serde_json::to_string(&message).unwrap_or_default();
                if ws_sender.send(Message::Text(json.into())).await.is_err() {
                    warn!(session_id = %session_id_for_send, "WebSocket送信失敗");
                    break;
                }
            }

            // セッションをクリーンアップ
            sessions_clone.write().await.remove(&session_id_for_send);
            info!(session_id = %session_id_for_send, "WebSocket送信タスク終了");
        });

        // 受信タスク（クライアント→サーバー）
        let session_id_for_recv = session_id.clone();
        let sessions_for_recv = self.sessions.clone();
        let recv_task = tokio::spawn(async move {
            while let Some(msg) = ws_receiver.next().await {
                match msg {
                    Ok(Message::Text(text)) => {
                        debug!(session_id = %session_id_for_recv, "受信: {}", text);

                        match serde_json::from_str::<SignalingMessage>(&text) {
                            Ok(signaling_msg) => {
                                // メッセージ処理
                                if let Err(e) =
                                    Self::process_message(&sessions_for_recv, signaling_msg).await
                                {
                                    error!(session_id = %session_id_for_recv, error = %e, "メッセージ処理失敗");
                                }
                            }
                            Err(e) => {
                                error!(session_id = %session_id_for_recv, error = %e, "JSON解析失敗");
                            }
                        }
                    }
                    Ok(Message::Close(_)) => {
                        info!(session_id = %session_id_for_recv, "WebSocket切断");
                        break;
                    }
                    Err(e) => {
                        error!(session_id = %session_id_for_recv, error = %e, "WebSocketエラー");
                        break;
                    }
                    _ => {}
                }
            }

            info!(session_id = %session_id_for_recv, "WebSocket受信タスク終了");
        });

        // 両タスク完了まで待機
        let _ = tokio::join!(send_task, recv_task);

        // セッションクリーンアップ
        self.sessions.write().await.remove(&session_id);
        info!(session_id = %session_id, "WebSocketシグナリング接続終了");
    }

    /// メッセージ処理
    async fn process_message(
        _sessions: &Arc<RwLock<HashMap<String, mpsc::Sender<SignalingMessage>>>>,
        message: SignalingMessage,
    ) -> Result<(), String> {
        match message {
            SignalingMessage::Offer { session_id, sdp: _ } => {
                debug!(session_id = %session_id, "Offer SDP受信");
                // ここでWebRTCトランスポートにSDPを渡して Answer を生成
                // 実装は別モジュールで統合
                Ok(())
            }
            SignalingMessage::IceCandidate {
                session_id,
                candidate: _,
            } => {
                debug!(session_id = %session_id, "ICE Candidate受信");
                // ICE Candidateを WebRTCトランスポートに追加
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// メッセージをセッションに送信
    pub async fn send_to_session(
        &self,
        session_id: &str,
        message: SignalingMessage,
    ) -> Result<(), String> {
        let sessions = self.sessions.read().await;
        if let Some(tx) = sessions.get(session_id) {
            tx.send(message)
                .await
                .map_err(|e| format!("メッセージ送信失敗: {}", e))?;
            Ok(())
        } else {
            Err(format!("セッション未登録: {}", session_id))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_signaling_handler_creation() {
        let handler = WebSocketSignalingHandler::new();
        assert_eq!(handler.sessions.try_read().unwrap().len(), 0);
    }

    #[test]
    fn test_signaling_message_serialization() {
        let msg = SignalingMessage::Offer {
            session_id: "test-123".to_string(),
            sdp: "v=0...".to_string(),
        };

        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("\"type\":\"offer\""));
        assert!(json.contains("test-123"));
    }
}

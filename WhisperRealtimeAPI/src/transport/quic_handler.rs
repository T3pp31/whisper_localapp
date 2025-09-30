use crate::transport::{StreamKind, TransportError};
use bytes::Bytes;
use parking_lot::RwLock;
use quinn::{Connection, Endpoint, RecvStream, SendStream, ServerConfig};
use rustls::pki_types::{CertificateDer, PrivateKeyDer};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info};

/// QUICストリームハンドラ
pub struct QuicStreamHandler {
    connections: Arc<RwLock<HashMap<String, Arc<QuicConnection>>>>,
    endpoint: Option<Endpoint>,
}

/// QUIC接続
pub struct QuicConnection {
    session_id: String,
    connection: Connection,
    streams: Arc<RwLock<HashMap<StreamKind, StreamChannels>>>,
}

/// ストリームチャネル
pub struct StreamChannels {
    tx: mpsc::Sender<Bytes>,
    rx: Arc<RwLock<Option<mpsc::Receiver<Bytes>>>>,
}

impl QuicStreamHandler {
    pub fn new() -> Self {
        Self {
            connections: Arc::new(RwLock::new(HashMap::new())),
            endpoint: None,
        }
    }

    /// QUICサーバーを起動
    pub async fn start_server(
        &mut self,
        addr: SocketAddr,
        cert: CertificateDer<'static>,
        key: PrivateKeyDer<'static>,
    ) -> Result<(), TransportError> {
        let mut server_config = ServerConfig::with_single_cert(vec![cert], key)
            .map_err(|e| TransportError::Internal { message: format!("証明書設定失敗: {}", e) })?;

        let transport_config = Arc::get_mut(&mut server_config.transport)
            .ok_or_else(|| TransportError::Internal { message: "トランスポート設定取得失敗".to_string() })?;

        // QUIC設定の調整
        transport_config
            .max_concurrent_bidi_streams(100u32.into())
            .max_concurrent_uni_streams(100u32.into())
            .max_idle_timeout(Some(std::time::Duration::from_secs(120).try_into().unwrap()));

        let endpoint = Endpoint::server(server_config, addr)
            .map_err(|e| TransportError::Internal { message: format!("QUICサーバー起動失敗: {}", e) })?;

        info!(addr = %addr, "QUICサーバー起動");

        self.endpoint = Some(endpoint);
        Ok(())
    }

    /// 接続を受け入れ
    pub async fn accept_connection(&self) -> Result<(String, Arc<QuicConnection>), TransportError> {
        let endpoint = self
            .endpoint
            .as_ref()
            .ok_or_else(|| TransportError::Internal { message: "QUICエンドポイント未初期化".to_string() })?;

        let incoming = endpoint
            .accept()
            .await
            .ok_or_else(|| TransportError::Internal { message: "接続受け入れ失敗".to_string() })?;

        let connection = incoming
            .await
            .map_err(|e| TransportError::Internal { message: format!("QUIC接続確立失敗: {}", e) })?;

        let session_id = uuid::Uuid::new_v4().to_string();
        let quic_conn = Arc::new(QuicConnection {
            session_id: session_id.clone(),
            connection,
            streams: Arc::new(RwLock::new(HashMap::new())),
        });

        self.connections
            .write()
            .insert(session_id.clone(), quic_conn.clone());

        info!(session_id = %session_id, "QUIC接続確立");

        Ok((session_id, quic_conn))
    }

    /// セッション取得
    pub fn get_connection(&self, session_id: &str) -> Option<Arc<QuicConnection>> {
        self.connections.read().get(session_id).cloned()
    }

    /// セッション終了
    pub fn close_connection(&self, session_id: &str) {
        if let Some(conn) = self.connections.write().remove(session_id) {
            conn.connection.close(0u32.into(), b"session closed");
            info!(session_id = %session_id, "QUIC接続終了");
        }
    }
}

impl QuicConnection {
    /// 双方向ストリームを開く
    pub async fn open_stream(&self, kind: StreamKind) -> Result<(), TransportError> {
        let (send_stream, recv_stream) = self
            .connection
            .open_bi()
            .await
            .map_err(|e| TransportError::Internal { message: format!("ストリーム開始失敗: {}", e) })?;

        let (tx, rx) = mpsc::channel::<Bytes>(100);

        let channels = StreamChannels {
            tx: tx.clone(),
            rx: Arc::new(RwLock::new(Some(rx))),
        };

        self.streams.write().insert(kind.clone(), channels);

        // 送信タスク
        let session_id = self.session_id.clone();
        tokio::spawn(stream_sender(session_id.clone(), kind.clone(), send_stream, tx));

        // 受信タスク
        tokio::spawn(stream_receiver(session_id, kind, recv_stream));

        Ok(())
    }

    /// ストリームへデータ送信
    pub async fn send_to_stream(
        &self,
        kind: &StreamKind,
        data: Bytes,
    ) -> Result<(), TransportError> {
        let streams = self.streams.read();
        let channels = streams
            .get(kind)
            .ok_or_else(|| TransportError::Internal { message: format!("ストリーム未開始: {:?}", kind) })?;

        channels
            .tx
            .send(data)
            .await
            .map_err(|e| TransportError::Internal { message: format!("データ送信失敗: {}", e) })?;

        Ok(())
    }

    /// ストリームからデータ受信チャネルを取得
    pub fn take_stream_rx(&self, kind: &StreamKind) -> Option<mpsc::Receiver<Bytes>> {
        let streams = self.streams.read();
        if let Some(channels) = streams.get(kind) {
            channels.rx.write().take()
        } else {
            None
        }
    }

    /// 接続を閉じる
    pub fn close(&self, code: u32, reason: &[u8]) {
        self.connection.close(code.into(), reason);
    }
}

/// ストリーム送信タスク
async fn stream_sender(
    session_id: String,
    kind: StreamKind,
    mut send_stream: SendStream,
    mut tx: mpsc::Sender<Bytes>,
) {
    debug!(session_id = %session_id, kind = ?kind, "ストリーム送信開始");

    // 実際の送信ロジックは別チャネルから受信したデータを送信
    // ここでは簡易的な実装
    loop {
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        // 実装: txから受信してsend_streamへ書き込み
    }
}

/// ストリーム受信タスク
async fn stream_receiver(session_id: String, kind: StreamKind, mut recv_stream: RecvStream) {
    debug!(session_id = %session_id, kind = ?kind, "ストリーム受信開始");

    loop {
        match recv_stream.read_chunk(8192, true).await {
            Ok(Some(chunk)) => {
                debug!(
                    session_id = %session_id,
                    kind = ?kind,
                    size = chunk.bytes.len(),
                    "データ受信"
                );
                // 実装: 受信データの処理
            }
            Ok(None) => {
                info!(session_id = %session_id, kind = ?kind, "ストリーム終了");
                break;
            }
            Err(e) => {
                error!(session_id = %session_id, kind = ?kind, error = %e, "受信エラー");
                break;
            }
        }
    }
}

/// 自己署名証明書を生成（テスト用）
pub fn generate_self_signed_cert() -> Result<(CertificateDer<'static>, PrivateKeyDer<'static>), TransportError> {
    let cert = rcgen::generate_simple_self_signed(vec!["localhost".to_string()])
        .map_err(|e| TransportError::Internal { message: format!("証明書生成失敗: {}", e) })?;

    let cert_der = CertificateDer::from(cert.cert.der().to_vec());
    let key_der = PrivateKeyDer::try_from(cert.key_pair.serialize_der())
        .map_err(|_| TransportError::Internal { message: "秘密鍵変換失敗".to_string() })?;

    Ok((cert_der, key_der))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quic_handler_creation() {
        let handler = QuicStreamHandler::new();
        assert_eq!(handler.connections.read().len(), 0);
    }

    #[test]
    fn test_generate_self_signed_cert() {
        let result = generate_self_signed_cert();
        assert!(result.is_ok());
    }
}
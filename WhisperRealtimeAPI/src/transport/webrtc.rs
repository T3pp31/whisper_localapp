use crate::transport::{ConnectionProfile, TransportError};
use bytes::Bytes;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::mpsc;
use tracing::{debug, error, info, warn};
use interceptor::registry::Registry;
use webrtc::api::interceptor_registry::register_default_interceptors;
use webrtc::api::media_engine::MediaEngine;
use webrtc::api::APIBuilder;
use webrtc::ice_transport::ice_server::RTCIceServer;
use webrtc::peer_connection::configuration::RTCConfiguration;
use webrtc::peer_connection::peer_connection_state::RTCPeerConnectionState;
use webrtc::peer_connection::sdp::session_description::RTCSessionDescription;
use webrtc::peer_connection::RTCPeerConnection;
use webrtc::rtp_transceiver::rtp_codec::RTCRtpCodecCapability;
use webrtc::track::track_remote::TrackRemote;

/// WebRTCトランスポートマネージャ
pub struct WebRtcTransport {
    sessions: Arc<RwLock<HashMap<String, Arc<WebRtcSession>>>>,
    ice_servers: Vec<RTCIceServer>,
}

/// WebRTCセッション
pub struct WebRtcSession {
    session_id: String,
    peer_connection: Arc<RTCPeerConnection>,
    audio_rx: Arc<RwLock<Option<mpsc::Receiver<Bytes>>>>,
    control_tx: mpsc::Sender<ControlMessage>,
    control_rx: Arc<RwLock<Option<mpsc::Receiver<ControlMessage>>>>,
}

#[derive(Debug, Clone)]
pub enum ControlMessage {
    BitrateChange(u32),
    Close,
}

impl WebRtcTransport {
    pub fn new(ice_servers: Vec<RTCIceServer>) -> Self {
        Self {
            sessions: Arc::new(RwLock::new(HashMap::new())),
            ice_servers,
        }
    }

    /// WebRTC PeerConnectionを作成
    async fn create_peer_connection(&self) -> Result<Arc<RTCPeerConnection>, TransportError> {
        let mut media_engine = MediaEngine::default();

        // Opusコーデックを登録
        media_engine
            .register_codec(
                RTCRtpCodecCapability {
                    mime_type: "audio/opus".to_owned(),
                    clock_rate: 48000,
                    channels: 2,
                    sdp_fmtp_line: "".to_owned(),
                    rtcp_feedback: vec![],
                },
                webrtc::rtp_transceiver::rtp_codec::RTPCodecType::Audio,
            )
            .map_err(|e| TransportError::Internal(format!("コーデック登録失敗: {}", e)))?;

        let mut registry = interceptor::registry::Registry::new();
        registry = register_default_interceptors(registry, &mut media_engine)
            .map_err(|e| TransportError::Internal(format!("インターセプター登録失敗: {}", e)))?;

        let api = APIBuilder::new()
            .with_media_engine(media_engine)
            .with_interceptor_registry(registry)
            .build();

        let config = RTCConfiguration {
            ice_servers: self.ice_servers.clone(),
            ..Default::default()
        };

        let peer_connection = Arc::new(
            api.new_peer_connection(config)
                .await
                .map_err(|e| TransportError::Internal(format!("PeerConnection作成失敗: {}", e)))?,
        );

        Ok(peer_connection)
    }

    /// セッションを開始
    pub async fn start_session(
        &self,
        session_id: String,
        _profile: ConnectionProfile,
    ) -> Result<Arc<WebRtcSession>, TransportError> {
        let peer_connection = self.create_peer_connection().await?;

        let (audio_tx, audio_rx) = mpsc::channel::<Bytes>(100);
        let (control_tx, control_rx) = mpsc::channel::<ControlMessage>(10);

        let session_id_clone = session_id.clone();

        // 音声トラック受信ハンドラ
        peer_connection
            .on_track(Box::new(move |track, _receiver, _transceiver| {
                let session_id = session_id_clone.clone();
                let audio_tx = audio_tx.clone();

                Box::pin(async move {
                    info!(
                        session_id = %session_id,
                        track_id = %track.id(),
                        "音声トラック受信開始"
                    );

                    if let Some(track_remote) = track.as_ref().downcast_ref::<TrackRemote>() {
                        tokio::spawn(process_audio_track(
                            session_id.clone(),
                            Arc::new(track_remote.clone()),
                            audio_tx,
                        ));
                    }
                })
            }))
            .await;

        // 接続状態監視
        let session_id_for_state = session_id.clone();
        peer_connection
            .on_peer_connection_state_change(Box::new(move |state| {
                let session_id = session_id_for_state.clone();
                Box::pin(async move {
                    info!(session_id = %session_id, state = ?state, "PeerConnection状態変化");

                    match state {
                        RTCPeerConnectionState::Failed | RTCPeerConnectionState::Closed => {
                            warn!(session_id = %session_id, "PeerConnection切断");
                        }
                        RTCPeerConnectionState::Connected => {
                            info!(session_id = %session_id, "PeerConnection確立");
                        }
                        _ => {}
                    }
                })
            }))
            .await;

        let session_id_for_ice = session_id.clone();
        peer_connection
            .on_ice_connection_state_change(Box::new(move |state| {
                let session_id = session_id_for_ice.clone();
                Box::pin(async move {
                    debug!(session_id = %session_id, ice_state = ?state, "ICE状態変化");
                })
            }))
            .await;

        let session = Arc::new(WebRtcSession {
            session_id: session_id.clone(),
            peer_connection,
            audio_rx: Arc::new(RwLock::new(Some(audio_rx))),
            control_tx,
            control_rx: Arc::new(RwLock::new(Some(control_rx))),
        });

        self.sessions.write().insert(session_id.clone(), session.clone());

        Ok(session)
    }

    /// SDP Offerを処理しAnswerを生成
    pub async fn handle_offer(
        &self,
        session_id: &str,
        offer_sdp: &str,
    ) -> Result<String, TransportError> {
        let sessions = self.sessions.read();
        let session = sessions
            .get(session_id)
            .ok_or_else(|| TransportError::SessionNotFound(session_id.to_string()))?;

        let offer = RTCSessionDescription::offer(offer_sdp.to_string())
            .map_err(|e| TransportError::Internal(format!("Offer SDP解析失敗: {}", e)))?;

        session
            .peer_connection
            .set_remote_description(offer)
            .await
            .map_err(|e| TransportError::Internal(format!("RemoteDescription設定失敗: {}", e)))?;

        let answer = session
            .peer_connection
            .create_answer(None)
            .await
            .map_err(|e| TransportError::Internal(format!("Answer作成失敗: {}", e)))?;

        session
            .peer_connection
            .set_local_description(answer.clone())
            .await
            .map_err(|e| TransportError::Internal(format!("LocalDescription設定失敗: {}", e)))?;

        Ok(answer.sdp)
    }

    /// ICE Candidateを追加
    pub async fn add_ice_candidate(
        &self,
        session_id: &str,
        candidate: &str,
    ) -> Result<(), TransportError> {
        let sessions = self.sessions.read();
        let session = sessions
            .get(session_id)
            .ok_or_else(|| TransportError::SessionNotFound(session_id.to_string()))?;

        let ice_candidate = webrtc::ice_transport::ice_candidate::RTCIceCandidateInit {
            candidate: candidate.to_string(),
            ..Default::default()
        };

        session
            .peer_connection
            .add_ice_candidate(ice_candidate)
            .await
            .map_err(|e| TransportError::Internal(format!("ICE Candidate追加失敗: {}", e)))?;

        Ok(())
    }

    /// セッション終了
    pub async fn end_session(&self, session_id: &str) -> Result<(), TransportError> {
        let session = self.sessions.write().remove(session_id);

        if let Some(session) = session {
            let _ = session.control_tx.send(ControlMessage::Close).await;
            session
                .peer_connection
                .close()
                .await
                .map_err(|e| TransportError::Internal(format!("PeerConnection終了失敗: {}", e)))?;
            info!(session_id = %session_id, "セッション終了");
        }

        Ok(())
    }

    /// セッション取得
    pub fn get_session(&self, session_id: &str) -> Option<Arc<WebRtcSession>> {
        self.sessions.read().get(session_id).cloned()
    }
}

impl WebRtcSession {
    /// 音声データ受信チャネルを取得
    pub fn take_audio_rx(&self) -> Option<mpsc::Receiver<Bytes>> {
        self.audio_rx.write().take()
    }

    /// 制御チャネル受信を取得
    pub fn take_control_rx(&self) -> Option<mpsc::Receiver<ControlMessage>> {
        self.control_rx.write().take()
    }

    /// ビットレート変更
    pub async fn set_bitrate(&self, bitrate_kbps: u32) -> Result<(), TransportError> {
        self.control_tx
            .send(ControlMessage::BitrateChange(bitrate_kbps))
            .await
            .map_err(|e| TransportError::Internal(format!("ビットレート変更送信失敗: {}", e)))?;
        Ok(())
    }
}

/// 音声トラック処理
async fn process_audio_track(
    session_id: String,
    track: Arc<TrackRemote>,
    audio_tx: mpsc::Sender<Bytes>,
) {
    info!(
        session_id = %session_id,
        track_id = %track.id(),
        "音声トラック処理開始"
    );

    loop {
        match track.read_rtp().await {
            Ok((rtp_packet, _attributes)) => {
                let payload = Bytes::copy_from_slice(&rtp_packet.payload);

                if audio_tx.send(payload).await.is_err() {
                    warn!(session_id = %session_id, "音声データ送信失敗（チャネル閉鎖）");
                    break;
                }
            }
            Err(e) => {
                error!(session_id = %session_id, error = %e, "RTPパケット読み取り失敗");
                break;
            }
        }
    }

    info!(session_id = %session_id, "音声トラック処理終了");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_webrtc_transport_creation() {
        let transport = WebRtcTransport::new(vec![]);
        assert_eq!(transport.sessions.read().len(), 0);
    }
}
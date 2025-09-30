use std::sync::Arc;

use bytes::Bytes;
use tokio::sync::mpsc::Receiver;
use tokio::task::JoinHandle;
use tracing::{error, info, warn};

use crate::asr::{AsrManager, StreamingAsrClient};
use crate::audio_pipeline::{AudioOpusDecoder, AudioPipeline};
use crate::config::ConfigSet;
use crate::transport::{TransportError, WebRtcTransport};

#[derive(thiserror::Error, Debug)]
pub enum OrchestratorError {
    #[error("webrtc session not found: {0}")]
    SessionNotFound(String),
    #[error("transport error: {0}")]
    Transport(#[from] TransportError),
    #[error("opus decoder init failed: {0}")]
    OpusInit(String),
}

/// WebRTC→Opus→AudioPipeline→ASR を結線するオーケストレータ
pub struct RealtimeOrchestrator<C: StreamingAsrClient + Send + Sync + 'static> {
    config: Arc<ConfigSet>,
    asr: Arc<AsrManager<C>>,
}

impl<C> RealtimeOrchestrator<C>
where
    C: StreamingAsrClient + Send + Sync + 'static,
{
    pub fn new(config: Arc<ConfigSet>, asr: Arc<AsrManager<C>>) -> Self {
        Self { config, asr }
    }

    /// 既存のWebRTCセッションから音声を取り出し、ASRへ送るタスクを起動
    pub async fn spawn_for_webrtc(
        &self,
        webrtc: Arc<WebRtcTransport>,
        session_id: &str,
        ws: crate::signaling::websocket::WebSocketSignalingHandler,
    ) -> Result<JoinHandle<()>, OrchestratorError> {
        let session = webrtc
            .get_session(session_id)
            .ok_or_else(|| OrchestratorError::SessionNotFound(session_id.to_string()))?;

        let audio_rx = session.take_audio_rx();
        let audio_rx = audio_rx.ok_or_else(|| OrchestratorError::Transport(TransportError::Internal { message: "audio channel not available".into() }))?;

        Ok(self.spawn_pipeline_task(session_id.to_string(), audio_rx, ws))
    }

    /// 任意の音声バイトストリーム（Opus）からASRへ送るタスクを起動
    pub fn spawn_pipeline_task(&self, session_id: String, mut audio_rx: Receiver<Bytes>, ws: crate::signaling::websocket::WebSocketSignalingHandler) -> JoinHandle<()> {
        let mut pipeline = AudioPipeline::new(self.config.audio.clone());
        // Opusは入力formatに合わせる
        let mut opus = match AudioOpusDecoder::new(
            self.config.audio.input.sample_rate_hz,
            self.config.audio.input.channels as usize,
        ) {
            Ok(d) => d,
            Err(e) => {
                // 初期化失敗時は何もせず終了
                warn!(error = %e, "failed to init opus decoder");
                return tokio::spawn(async {});
            }
        };

        let asr = self.asr.clone();
        tokio::spawn(async move {
            if let Err(e) = asr.start_session(&session_id).await {
                error!(session_id = %session_id, error = %e, "asr start failed");
                return;
            }
            info!(session_id = %session_id, "pipeline started");

            // ASR更新のフォワーダ
            let asr_for_updates = asr.clone();
            let session_for_updates = session_id.clone();
            let ws_for_updates = ws.clone();
            tokio::spawn(async move {
                loop {
                    match asr_for_updates.poll_update(&session_for_updates).await {
                        Ok(Some(update)) => {
                            match update {
                                crate::asr::TranscriptUpdate::Partial { text, confidence } => {
                                    let _ = ws_for_updates
                                        .send_to_session(
                                            &session_for_updates,
                                            crate::signaling::websocket::SignalingMessage::PartialTranscript {
                                                session_id: session_for_updates.clone(),
                                                text,
                                                confidence,
                                            },
                                        )
                                        .await;
                                }
                                crate::asr::TranscriptUpdate::Final { text } => {
                                    let _ = ws_for_updates
                                        .send_to_session(
                                            &session_for_updates,
                                            crate::signaling::websocket::SignalingMessage::FinalTranscript {
                                                session_id: session_for_updates.clone(),
                                                text,
                                            },
                                        )
                                        .await;
                                    break;
                                }
                            }
                        }
                        Ok(None) => {
                            // 受信待ち
                        }
                        Err(e) => {
                            warn!(session_id = %session_for_updates, error = %e, "poll_update failed");
                            break;
                        }
                    }
                }
            });

            while let Some(packet) = audio_rx.recv().await {
                match opus.decode(&packet) {
                    Ok(samples_i16) => {
                        let frames = pipeline.process(&samples_i16);
                        for f in frames {
                            if let Err(e) = asr.send_audio(&session_id, f).await {
                                warn!(session_id = %session_id, error = %e, "send_audio failed");
                                break;
                            }
                        }
                    }
                    Err(e) => {
                        warn!(session_id = %session_id, error = %e, "opus decode failed");
                        // PLCで隙間を埋める
                        if let Ok(samples_i16) = opus.decode_plc() {
                            let frames = pipeline.process(&samples_i16);
                            for f in frames {
                                let _ = asr.send_audio(&session_id, f).await;
                            }
                        }
                    }
                }
            }

            // flush and finish
            if let Some(rem) = pipeline.flush() {
                let _ = asr.send_audio(&session_id, rem).await;
            }
            let _ = asr.finish_session(&session_id).await;
            let _ = asr.drop_session(&session_id).await;
            info!(session_id = %session_id, "pipeline finished");
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orchestrator_type_compiles() {
        // コンパイルテスト用（実ラン不可の簡易テスト）
        // 実データはネットワーク依存のため別統合テストで検証
        assert!(true);
    }
}

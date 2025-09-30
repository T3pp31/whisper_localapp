use std::sync::Arc;

use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use whisper_realtime_api::asr::{AsrManager, GrpcAsrClient, grpc_client::GrpcAsrClientAdapter};
use whisper_realtime_api::audio_pipeline::AudioPipeline;
use whisper_realtime_api::config::ConfigSet;
use whisper_realtime_api::signaling::SignalingService;
use whisper_realtime_api::signaling::websocket::WebSocketSignalingHandler;
use whisper_realtime_api::server;
use whisper_realtime_api::transport::{ConnectionProfile, InMemoryTransport, StreamKind, WebRtcTransport};
use whisper_realtime_api::realtime::RealtimeOrchestrator;

#[tokio::main]
async fn main() {
    init_tracing();

    match ConfigSet::load_from_env() {
        Ok(config) => {
            let config = Arc::new(config);
            info!(root = ?config.root(), "configuration loaded");

            let signaling = SignalingService::with_default_validator(config.clone());
            let transport = InMemoryTransport::default();
            // WebRTCトランスポート
            let rtc_ice: Vec<webrtc::ice_transport::ice_server::RTCIceServer> = config
                .system
                .signaling
                .ice_servers
                .iter()
                .map(|s| webrtc::ice_transport::ice_server::RTCIceServer {
                    urls: s.urls.clone(),
                    username: s.username.clone().unwrap_or_default(),
                    credential: s.credential.clone().unwrap_or_default(),
                    credential_type: webrtc::ice_transport::ice_credential_type::RTCIceCredentialType::Unspecified,
                })
                .collect();
            let webrtc = Arc::new(WebRtcTransport::new(rtc_ice));
            let asr_config = Arc::new(config.asr.clone());
            let target_sr = config.audio.target.sample_rate_hz as i32;
            let target_ch = config.audio.target.channels as i32;
            let grpc_client = GrpcAsrClient::new(
                asr_config.service.endpoint.clone(),
                asr_config.clone(),
                target_sr,
                target_ch,
            );
            let asr_manager = AsrManager::new(GrpcAsrClientAdapter::from_client(grpc_client), asr_config);
            let mut audio_pipeline = AudioPipeline::new(config.audio.clone());

            info!(
                max_sessions = config.system.resources.max_concurrent_sessions,
                "signaling service initialized"
            );

            let default_profile = ConnectionProfile::new(
                config.system.signaling.default_bitrate_kbps,
                120,
                [
                    StreamKind::Audio,
                    StreamKind::PartialTranscript,
                    StreamKind::FinalTranscript,
                    StreamKind::Control,
                ],
            );
            info!(
                bitrate = default_profile.max_bitrate_kbps,
                "default QUIC profile prepared"
            );

            let input_frame_samples = (config.audio.input.sample_rate_hz as usize
                * config.audio.frame_assembler.frame_duration_ms as usize)
                / 1000;
            let silent_frame =
                vec![0_i16; input_frame_samples * config.audio.input.channels as usize];
            let frames = audio_pipeline.process(&silent_frame);
            info!(
                produced_frames = frames.len(),
                "audio pipeline warm-up complete"
            );

            // WebSocketシグナリングサーバ起動
            // WebSocketハンドラと受信チャネル
            let (incoming_tx, mut incoming_rx) = tokio::sync::mpsc::channel(256);
            let ws_handler = WebSocketSignalingHandler::with_channel(incoming_tx.clone());

            // オーケストレータ
            let orchestrator = Arc::new(RealtimeOrchestrator::new(config.clone(), Arc::new(asr_manager)));

            let ws_addr = config.server.ws_bind_addr.clone();
            info!(addr = %ws_addr, "starting websocket signaling server");
            let ws_handler_for_send = ws_handler.clone();
            let server_task = tokio::spawn(async move {
                if let Err(e) = server::bind_and_run(&ws_addr, ws_handler).await {
                    error!(error = %e, "failed to start server");
                }
            });

            // シグナリング受信をアプリ層で処理（メインタスク側で実行）
            let default_profile = ConnectionProfile::new(
                config.system.signaling.default_bitrate_kbps,
                120,
                [
                    StreamKind::Audio,
                    StreamKind::PartialTranscript,
                    StreamKind::FinalTranscript,
                    StreamKind::Control,
                ],
            );
            while let Some(msg) = incoming_rx.recv().await {
                match msg {
                    whisper_realtime_api::signaling::websocket::SignalingMessage::Offer { session_id, sdp } => {
                        let _ = webrtc.start_session(session_id.clone(), default_profile.clone()).await;
                        if let Ok(answer) = webrtc.handle_offer(&session_id, &sdp).await {
                            let _ = ws_handler_for_send
                                .send_to_session(&session_id, whisper_realtime_api::signaling::websocket::SignalingMessage::Answer { session_id: session_id.clone(), sdp: answer })
                                .await;
                        }
                        let _ = orchestrator.spawn_for_webrtc(webrtc.clone(), &session_id, ws_handler_for_send.clone()).await;
                    }
                    whisper_realtime_api::signaling::websocket::SignalingMessage::IceCandidate { session_id, candidate } => {
                        let _ = webrtc.add_ice_candidate(&session_id, &candidate).await;
                    }
                    _ => {}
                }
            }

            // 終了待機
            let _ = server_task.await;

            let _ = (signaling, transport);
        }
        Err(err) => {
            error!(error = ?err, "failed to load configuration");
            std::process::exit(1);
        }
    }
}

fn init_tracing() {
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .finish();

    if let Err(err) = tracing::subscriber::set_global_default(subscriber) {
        eprintln!("failed to install tracing subscriber: {err}");
    }
}

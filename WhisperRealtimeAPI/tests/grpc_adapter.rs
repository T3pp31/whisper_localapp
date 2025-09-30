use std::sync::Arc;

use whisper_realtime_api::asr::{GrpcAsrClient, grpc_client::GrpcAsrClientAdapter, AsrManager};
use whisper_realtime_api::config::ConfigSet;

#[tokio::test]
async fn adapter_start_session_allows_send_and_finish() {
    let cfg = ConfigSet::load_from_dir("config").expect("cfg");
    let asr_cfg = Arc::new(cfg.asr.clone());

    let client = GrpcAsrClient::new(
        asr_cfg.service.endpoint.clone(),
        asr_cfg.clone(),
        cfg.audio.target.sample_rate_hz as i32,
        cfg.audio.target.channels as i32,
    );
    let manager = AsrManager::new(GrpcAsrClientAdapter::from_client(client), asr_cfg);

    let sid = "adapter-session-1";
    manager.start_session(sid).await.expect("start");
    manager.send_audio(sid, vec![0.0_f32; 160]).await.expect("send");
    manager.finish_session(sid).await.expect("finish");
    // 一回はポーリングできる（Finalが来る想定）
    let _ = manager.poll_update(sid).await.expect("poll");
    manager.drop_session(sid).await.expect("drop");
}


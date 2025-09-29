use std::sync::Arc;

use whisper_realtime_api::asr::{AsrManager, MockAsrClient, TranscriptUpdate};
use whisper_realtime_api::config::ConfigSet;

#[tokio::test]
async fn streaming_session_emits_partial_and_final() {
    let config = ConfigSet::load_from_dir("config").expect("config");
    let asr_config = Arc::new(config.asr.clone());
    let manager = AsrManager::new(MockAsrClient::new(asr_config.clone()), asr_config);

    let session_id = "session-asr";
    manager
        .start_session(session_id)
        .await
        .expect("session start");
    manager
        .send_audio(session_id, vec![0.1, 0.2, 0.3])
        .await
        .expect("send audio");
    manager
        .send_audio(session_id, vec![0.4, 0.5, 0.6])
        .await
        .expect("send audio");
    manager.finish_session(session_id).await.expect("finish");

    let mut received_final = false;
    for _ in 0..10 {
        if let Some(update) = manager.poll_update(session_id).await.expect("poll") {
            match update {
                TranscriptUpdate::Partial { .. } => {}
                TranscriptUpdate::Final { .. } => {
                    received_final = true;
                    break;
                }
            }
        }
    }

    assert!(received_final, "final transcript should arrive");
    manager
        .drop_session(session_id)
        .await
        .expect("drop session");
}

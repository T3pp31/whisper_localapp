use std::sync::Arc;

use tokio::time::{timeout, Duration};

use whisper_realtime_api::asr::{AsrManager, MockAsrClient, TranscriptUpdate};
use whisper_realtime_api::config::ConfigSet;
use whisper_realtime_api::ingest::PcmIngestor;

#[tokio::test]
async fn pcm_ingest_partial_and_final() {
    let cfg = ConfigSet::load_from_env().expect("load config");
    let asr_cfg = Arc::new(cfg.asr.clone());
    let manager = Arc::new(AsrManager::new(MockAsrClient::new(asr_cfg.clone()), asr_cfg));

    let ingestor = PcmIngestor::new(manager.clone(), cfg.audio.clone());

    let session_id = "sess-ingest-1";
    ingestor.start_session(session_id).await.expect("start session");

    // 入力の1チャンク: input.sample_rate * frame_ms / 1000 * channels のS16LE
    let input_sr = cfg.audio.input.sample_rate_hz as usize;
    let channels = cfg.audio.input.channels as usize;
    let frame_ms = cfg.audio.frame_assembler.frame_duration_ms as usize;
    let samples_per_frame = input_sr * frame_ms / 1000;
    let interleaved = vec![0_i16; samples_per_frame * channels];

    // 複数チャンク投入
    ingestor
        .ingest_chunk(session_id, &interleaved)
        .await
        .expect("ingest#1");
    ingestor
        .ingest_chunk(session_id, &interleaved)
        .await
        .expect("ingest#2");

    // セッション終了
    ingestor
        .finish_session(session_id)
        .await
        .expect("finish");

    // ある程度の時間内にPartial/Finalが出ること
    let mut got_partial = false;
    let mut got_final = false;

    // updateをポーリング（Mockは即時返す想定だが、保険でタイムアウト）
    let _ = timeout(Duration::from_millis(500), async {
        loop {
            match manager.poll_update(session_id).await {
                Ok(Some(TranscriptUpdate::Partial { .. })) => {
                    got_partial = true;
                }
                Ok(Some(TranscriptUpdate::Final { .. })) => {
                    got_final = true;
                    break;
                }
                Ok(None) => {
                    // 少し待つ
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Err(_) => break,
            }
        }
    })
    .await;

    assert!(got_partial, "partial transcript should arrive");
    assert!(got_final, "final transcript should arrive");
}


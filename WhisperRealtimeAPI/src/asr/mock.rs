use std::sync::Arc;

use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::config::AsrPipelineConfig;

use super::client::{AudioCommand, StreamingAsrClient, StreamingSession, TranscriptUpdate};
use super::error::AsrError;

#[derive(Debug, Clone)]
pub struct MockAsrClient {
    config: Arc<AsrPipelineConfig>,
}

impl MockAsrClient {
    pub fn new(config: Arc<AsrPipelineConfig>) -> Self {
        Self { config }
    }
}

impl StreamingAsrClient for MockAsrClient {
    fn start_session(&self, session_id: &str) -> Result<StreamingSession, AsrError> {
        let (command_tx, mut command_rx) = mpsc::channel::<AudioCommand>(32);
        let (update_tx, update_rx) = mpsc::channel::<TranscriptUpdate>(32);
        let session_id = session_id.to_string();
        let session_id_for_task = session_id.clone();
        let config = self.config.clone();

        let flush_interval = config.streaming.max_pending_requests.max(1);

        let _worker: JoinHandle<()> = tokio::spawn(async move {
            let mut partial_accumulator = String::new();
            let mut frame_index = 0_u32;
            while let Some(command) = command_rx.recv().await {
                match command {
                    AudioCommand::Frame(samples) => {
                        frame_index += 1;
                        partial_accumulator.push_str(&format!(" {}", samples.len()));
                        let _ = update_tx
                            .send(TranscriptUpdate::Partial {
                                text: format!(
                                    "session {} frame {} samples {}",
                                    session_id_for_task,
                                    frame_index,
                                    samples.len()
                                ),
                                confidence: 0.8,
                            })
                            .await;
                        if frame_index % flush_interval == 0 {
                            let _ = update_tx
                                .send(TranscriptUpdate::Partial {
                                    text: format!(
                                        "session {} aggregated{}",
                                        session_id_for_task, partial_accumulator
                                    ),
                                    confidence: 0.9,
                                })
                                .await;
                            partial_accumulator.clear();
                        }
                    }
                    AudioCommand::Finish => {
                        let summary = if partial_accumulator.is_empty() {
                            String::from(" no additional data")
                        } else {
                            format!(" with{}", partial_accumulator)
                        };
                        let final_text =
                            format!("session {} complete{}", session_id_for_task, summary);
                        let _ = update_tx
                            .send(TranscriptUpdate::Final { text: final_text })
                            .await;
                        break;
                    }
                }
            }
        });

        Ok(StreamingSession::new(session_id, command_tx, update_rx))
    }
}

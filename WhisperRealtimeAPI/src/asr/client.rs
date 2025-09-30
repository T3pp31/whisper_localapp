use tokio::sync::mpsc;

use super::error::AsrError;

#[derive(Debug, Clone, PartialEq)]
pub enum TranscriptUpdate {
    Partial { text: String, confidence: f32 },
    Final { text: String },
}

#[derive(Debug)]
pub struct StreamingSession {
    session_id: String,
    command_tx: mpsc::Sender<AudioCommand>,
    update_rx: mpsc::Receiver<TranscriptUpdate>,
}

#[derive(Debug)]
pub(crate) enum AudioCommand {
    Frame(Vec<f32>),
    Finish,
}

impl StreamingSession {
    pub(crate) fn new(
        session_id: impl Into<String>,
        command_tx: mpsc::Sender<AudioCommand>,
        update_rx: mpsc::Receiver<TranscriptUpdate>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            command_tx,
            update_rx,
        }
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub async fn send_audio(&self, frame: Vec<f32>) -> Result<(), AsrError> {
        self.command_tx
            .send(AudioCommand::Frame(frame))
            .await
            .map_err(|_| AsrError::Processing {
                message: "audio command channel closed".to_string(),
            })
    }

    pub async fn finish(&self) -> Result<(), AsrError> {
        self.command_tx
            .send(AudioCommand::Finish)
            .await
            .map_err(|_| AsrError::Processing {
                message: "audio command channel closed".to_string(),
            })
    }

    pub async fn next_update(&mut self) -> Option<TranscriptUpdate> {
        self.update_rx.recv().await
    }
}

pub trait StreamingAsrClient: Send + Sync {
    fn start_session(&self, session_id: &str) -> Result<StreamingSession, AsrError>;
}

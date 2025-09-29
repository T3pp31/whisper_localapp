mod client;
mod error;
mod mock;

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{Mutex, RwLock};

use crate::config::AsrPipelineConfig;

pub use client::{StreamingAsrClient, StreamingSession, TranscriptUpdate};
pub use error::AsrError;
pub use mock::MockAsrClient;

pub struct AsrManager<C>
where
    C: StreamingAsrClient + Send + Sync + 'static,
{
    client: C,
    sessions: RwLock<HashMap<String, Arc<Mutex<StreamingSession>>>>,
    config: Arc<AsrPipelineConfig>,
}

impl<C> AsrManager<C>
where
    C: StreamingAsrClient + Send + Sync + 'static,
{
    pub fn new(client: C, config: Arc<AsrPipelineConfig>) -> Self {
        Self {
            client,
            sessions: RwLock::new(HashMap::new()),
            config,
        }
    }

    pub async fn start_session(&self, session_id: &str) -> Result<(), AsrError> {
        let session = self.client.start_session(session_id)?;
        let mut guard = self.sessions.write().await;
        guard.insert(session_id.to_string(), Arc::new(Mutex::new(session)));
        Ok(())
    }

    pub async fn send_audio(&self, session_id: &str, frame: Vec<f32>) -> Result<(), AsrError> {
        let sessions = self.sessions.read().await;
        let session = sessions
            .get(session_id)
            .ok_or_else(|| AsrError::StreamNotFound {
                session_id: session_id.to_string(),
            })?
            .clone();
        let guard = session.lock().await;
        guard.send_audio(frame).await
    }

    pub async fn finish_session(&self, session_id: &str) -> Result<(), AsrError> {
        let sessions = self.sessions.read().await;
        let session = sessions
            .get(session_id)
            .ok_or_else(|| AsrError::StreamNotFound {
                session_id: session_id.to_string(),
            })?
            .clone();
        session.lock().await.finish().await
    }

    pub async fn poll_update(
        &self,
        session_id: &str,
    ) -> Result<Option<TranscriptUpdate>, AsrError> {
        let sessions = self.sessions.read().await;
        let session = sessions
            .get(session_id)
            .ok_or_else(|| AsrError::StreamNotFound {
                session_id: session_id.to_string(),
            })?
            .clone();
        let mut guard = session.lock().await;
        Ok(guard.next_update().await)
    }

    pub async fn drop_session(&self, session_id: &str) -> Result<(), AsrError> {
        let mut sessions = self.sessions.write().await;
        sessions
            .remove(session_id)
            .map(|_| ())
            .ok_or_else(|| AsrError::StreamNotFound {
                session_id: session_id.to_string(),
            })
    }

    pub fn config(&self) -> Arc<AsrPipelineConfig> {
        self.config.clone()
    }
}

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::{RwLock, mpsc};

use super::profile::StreamKind;
use super::{ConnectionProfile, QuicTransport, StreamHandle, TransportError, TransportSession};

#[derive(Debug, Default)]
pub struct InMemoryTransport {
    sessions: RwLock<HashMap<String, Arc<SessionState>>>,
}

#[derive(Debug)]
struct SessionState {
    session: Arc<TransportSession>,
    bandwidth_limit: RwLock<u32>,
}

#[async_trait]
impl QuicTransport for InMemoryTransport {
    async fn connect(
        &self,
        session_id: &str,
        profile: ConnectionProfile,
    ) -> Result<Arc<TransportSession>, TransportError> {
        let mut sessions = self.sessions.write().await;
        if sessions.contains_key(session_id) {
            return Err(TransportError::AlreadyConnected {
                session_id: session_id.to_string(),
            });
        }

        let streams = create_streams(&profile);
        let transport_session = Arc::new(TransportSession::new(session_id, streams));
        let state = Arc::new(SessionState {
            session: transport_session.clone(),
            bandwidth_limit: RwLock::new(profile.max_bitrate_kbps),
        });

        sessions.insert(session_id.to_string(), state);
        Ok(transport_session)
    }

    async fn disconnect(&self, session_id: &str) -> Result<(), TransportError> {
        let mut sessions = self.sessions.write().await;
        sessions
            .remove(session_id)
            .map(|_| ())
            .ok_or_else(|| TransportError::NotFound {
                session_id: session_id.to_string(),
            })
    }

    async fn apply_bandwidth_limit(
        &self,
        session_id: &str,
        max_bitrate_kbps: u32,
    ) -> Result<(), TransportError> {
        let sessions = self.sessions.read().await;
        let state = sessions
            .get(session_id)
            .ok_or_else(|| TransportError::NotFound {
                session_id: session_id.to_string(),
            })?;
        let mut guard = state.bandwidth_limit.write().await;
        *guard = max_bitrate_kbps;
        Ok(())
    }
}

impl InMemoryTransport {
    pub async fn bandwidth_limit(&self, session_id: &str) -> Result<u32, TransportError> {
        let sessions = self.sessions.read().await;
        let state = sessions
            .get(session_id)
            .ok_or_else(|| TransportError::NotFound {
                session_id: session_id.to_string(),
            })?;
        let guard = state.bandwidth_limit.read().await;
        Ok(*guard)
    }

    pub async fn session(&self, session_id: &str) -> Result<Arc<TransportSession>, TransportError> {
        let sessions = self.sessions.read().await;
        sessions
            .get(session_id)
            .map(|state| state.session.clone())
            .ok_or_else(|| TransportError::NotFound {
                session_id: session_id.to_string(),
            })
    }
}

fn create_streams(profile: &ConnectionProfile) -> HashMap<StreamKind, Arc<StreamHandle>> {
    let mut streams = HashMap::new();
    for kind in profile.stream_kinds.iter().copied() {
        let (tx, rx) = mpsc::channel(128);
        let handle = Arc::new(StreamHandle::new(kind, tx, rx));
        streams.insert(kind, handle);
    }
    streams
}

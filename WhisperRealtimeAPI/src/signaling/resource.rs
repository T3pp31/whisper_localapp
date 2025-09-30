use std::collections::HashMap;
use std::time::{Duration, Instant};

use tokio::sync::RwLock;

use super::error::SignalingError;
use super::session::SessionHandle;

#[derive(Debug)]
pub struct ResourceManager {
    max_sessions: u32,
    session_timeout: Duration,
    sessions: RwLock<HashMap<String, ResourceEntry>>,
}

#[derive(Debug)]
struct ResourceEntry {
    handle: SessionHandle,
    last_heartbeat: Instant,
}

impl ResourceManager {
    pub fn new(max_sessions: u32, session_timeout_s: u64) -> Self {
        Self {
            max_sessions,
            session_timeout: Duration::from_secs(session_timeout_s),
            sessions: RwLock::new(HashMap::new()),
        }
    }

    pub async fn try_allocate(&self, handle: SessionHandle) -> Result<(), SignalingError> {
        let mut guard = self.sessions.write().await;
        guard.retain(|_, entry| entry.last_heartbeat.elapsed() <= self.session_timeout);
        if guard.len() >= self.max_sessions as usize {
            return Err(SignalingError::ResourceLimitExceeded);
        }
        guard.insert(
            handle.id.clone(),
            ResourceEntry {
                handle,
                last_heartbeat: Instant::now(),
            },
        );
        Ok(())
    }

    pub async fn heartbeat(&self, session_id: &str) -> Result<(), SignalingError> {
        let mut guard = self.sessions.write().await;
        match guard.get_mut(session_id) {
            Some(entry) => {
                entry.last_heartbeat = Instant::now();
                Ok(())
            }
            None => Err(SignalingError::SessionNotFound {
                session_id: session_id.to_string(),
            }),
        }
    }

    pub async fn release(&self, session_id: &str) -> Result<(), SignalingError> {
        let mut guard = self.sessions.write().await;
        guard
            .remove(session_id)
            .map(|_| ())
            .ok_or_else(|| SignalingError::SessionNotFound {
                session_id: session_id.to_string(),
            })
    }

    pub async fn active_sessions(&self) -> usize {
        let guard = self.sessions.read().await;
        guard.len()
    }

    pub fn max_sessions(&self) -> u32 {
        self.max_sessions
    }

    pub async fn session_handles(&self) -> Vec<SessionHandle> {
        let guard = self.sessions.read().await;
        guard.values().map(|entry| entry.handle.clone()).collect()
    }
}

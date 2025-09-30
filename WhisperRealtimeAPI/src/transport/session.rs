use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{RwLock, mpsc};

use super::error::TransportError;
use super::profile::StreamKind;

#[derive(Debug)]
pub struct StreamHandle {
    kind: StreamKind,
    sender: mpsc::Sender<Vec<u8>>,
    receiver: RwLock<mpsc::Receiver<Vec<u8>>>,
}

impl StreamHandle {
    pub fn new(
        kind: StreamKind,
        sender: mpsc::Sender<Vec<u8>>,
        receiver: mpsc::Receiver<Vec<u8>>,
    ) -> Self {
        Self {
            kind,
            sender,
            receiver: RwLock::new(receiver),
        }
    }

    pub fn kind(&self) -> StreamKind {
        self.kind
    }

    pub async fn send(&self, payload: Vec<u8>) -> Result<(), TransportError> {
        self.sender
            .send(payload)
            .await
            .map_err(|_| TransportError::Send)
    }

    pub async fn recv(&self) -> Option<Vec<u8>> {
        self.receiver.write().await.recv().await
    }
}

#[derive(Debug)]
pub struct TransportSession {
    session_id: String,
    streams: HashMap<StreamKind, Arc<StreamHandle>>,
}

impl TransportSession {
    pub fn new(
        session_id: impl Into<String>,
        streams: HashMap<StreamKind, Arc<StreamHandle>>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            streams,
        }
    }

    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    pub fn stream(&self, kind: StreamKind) -> Option<Arc<StreamHandle>> {
        self.streams.get(&kind).cloned()
    }

    pub fn streams(&self) -> impl Iterator<Item = Arc<StreamHandle>> + '_ {
        self.streams.values().cloned()
    }
}

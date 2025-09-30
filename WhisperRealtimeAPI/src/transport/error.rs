use thiserror::Error;

#[derive(Debug, Error)]
pub enum TransportError {
    #[error("transport already connected for session {session_id}")]
    AlreadyConnected { session_id: String },
    #[error("transport not found for session {session_id}")]
    NotFound { session_id: String },
    #[error("channel send failed")]
    Send,
    #[error("internal transport error: {message}")]
    Internal { message: String },
}

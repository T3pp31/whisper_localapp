use thiserror::Error;

#[derive(Debug, Error)]
pub enum AsrError {
    #[error("connection failed: {message}")]
    Connection { message: String },
    #[error("stream not found for session {session_id}")]
    StreamNotFound { session_id: String },
    #[error("transcript processing failed: {message}")]
    Processing { message: String },
}

use thiserror::Error;

#[derive(Debug, Error)]
pub enum SignalingError {
    #[error("authentication failed: {reason}")]
    Authentication { reason: String },
    #[error("client not supported: {reason}")]
    ClientNotSupported { reason: String },
    #[error("resource limit exceeded")]
    ResourceLimitExceeded,
    #[error("session not found: {session_id}")]
    SessionNotFound { session_id: String },
    #[error("internal error: {message}")]
    Internal { message: String },
}

impl SignalingError {
    pub fn authentication(reason: impl Into<String>) -> Self {
        Self::Authentication {
            reason: reason.into(),
        }
    }

    pub fn client_not_supported(reason: impl Into<String>) -> Self {
        Self::ClientNotSupported {
            reason: reason.into(),
        }
    }

    pub fn internal(message: impl Into<String>) -> Self {
        Self::Internal {
            message: message.into(),
        }
    }
}

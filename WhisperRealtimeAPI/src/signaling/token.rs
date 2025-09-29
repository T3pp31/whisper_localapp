use std::time::{Duration, Instant};

use async_trait::async_trait;

use super::error::SignalingError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TokenClaims {
    pub subject: String,
    pub audience: String,
    pub expires_at: Option<Instant>,
}

#[async_trait]
pub trait TokenValidator: Send + Sync {
    async fn validate(
        &self,
        token: &str,
        expected_audience: &str,
    ) -> Result<TokenClaims, SignalingError>;
}

#[derive(Debug, Clone, Default)]
pub struct NoopTokenValidator;

#[async_trait]
impl TokenValidator for NoopTokenValidator {
    async fn validate(
        &self,
        token: &str,
        expected_audience: &str,
    ) -> Result<TokenClaims, SignalingError> {
        if token.is_empty() {
            return Err(SignalingError::authentication("empty token"));
        }

        let mut parts = token.splitn(2, ':');
        let audience = parts
            .next()
            .ok_or_else(|| SignalingError::authentication("missing audience"))?;
        let subject = parts
            .next()
            .ok_or_else(|| SignalingError::authentication("missing subject"))?;

        if audience != expected_audience {
            return Err(SignalingError::authentication("audience mismatch"));
        }

        Ok(TokenClaims {
            subject: subject.to_string(),
            audience: audience.to_string(),
            expires_at: Some(Instant::now() + Duration::from_secs(3600)),
        })
    }
}

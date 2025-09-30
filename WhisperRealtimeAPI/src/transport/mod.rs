mod error;
mod in_memory;
mod profile;
mod session;
pub mod quic_handler;
pub mod webrtc;

use std::sync::Arc;

use async_trait::async_trait;

pub use error::TransportError;
pub use in_memory::InMemoryTransport;
pub use profile::{ConnectionProfile, StreamKind};
pub use quic_handler::{QuicConnection, QuicStreamHandler};
pub use session::{StreamHandle, TransportSession};
pub use webrtc::WebRtcTransport;

#[async_trait]
pub trait QuicTransport: Send + Sync {
    async fn connect(
        &self,
        session_id: &str,
        profile: ConnectionProfile,
    ) -> Result<Arc<TransportSession>, TransportError>;

    async fn disconnect(&self, session_id: &str) -> Result<(), TransportError>;

    async fn apply_bandwidth_limit(
        &self,
        session_id: &str,
        max_bitrate_kbps: u32,
    ) -> Result<(), TransportError>;
}

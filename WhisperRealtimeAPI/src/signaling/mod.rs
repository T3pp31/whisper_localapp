mod error;
mod resource;
mod session;
mod token;
pub mod websocket;

use std::sync::Arc;

use tokio::sync::RwLock;

use crate::config::{ConfigSet, IceServerConfig};

pub use error::SignalingError;
pub use resource::ResourceManager;
pub use session::{
    ClientMetadata, ClientType, IceServer, SessionHandle, SessionRequest, SessionResponse,
};
pub use token::{NoopTokenValidator, TokenClaims, TokenValidator};
pub use websocket::{SignalingMessage, WebSocketSignalingHandler};

#[derive(Clone)]
pub struct SignalingService<V>
where
    V: TokenValidator + 'static,
{
    config: Arc<ConfigSet>,
    validator: V,
    resources: Arc<ResourceManager>,
    ice_servers: Arc<RwLock<Vec<IceServer>>>,
}

impl<V> SignalingService<V>
where
    V: TokenValidator + Send + Sync + 'static,
{
    pub fn new(config: Arc<ConfigSet>, validator: V) -> Self {
        let ice_servers = config
            .system
            .signaling
            .ice_servers
            .iter()
            .map(IceServer::from)
            .collect();

        let resources = Arc::new(ResourceManager::new(
            config.system.resources.max_concurrent_sessions,
            config.system.resources.session_timeout_s,
        ));

        Self {
            config,
            validator,
            resources,
            ice_servers: Arc::new(RwLock::new(ice_servers)),
        }
    }

    pub async fn start_session(
        &self,
        request: SessionRequest,
    ) -> Result<SessionResponse, SignalingError> {
        if !self.is_client_supported(&request.client) {
            return Err(SignalingError::client_not_supported("unsupported client"));
        }

        let expected_audience = &self.config.system.token.audience;
        let claims = self
            .validator
            .validate(&request.auth_token, expected_audience)
            .await?;

        let session_id = uuid::Uuid::new_v4().to_string();
        let handle = SessionHandle::new(session_id.clone(), request.client.clone(), claims.subject);
        self.resources.try_allocate(handle).await?;

        let ice_servers = self.current_ice_servers().await;
        let response = SessionResponse {
            session_id,
            ice_servers,
            max_bitrate_kbps: self.config.system.signaling.default_bitrate_kbps,
        };

        Ok(response)
    }

    pub async fn end_session(&self, session_id: &str) -> Result<(), SignalingError> {
        self.resources.release(session_id).await
    }

    pub async fn heartbeat(&self, session_id: &str) -> Result<(), SignalingError> {
        self.resources.heartbeat(session_id).await
    }

    pub async fn active_sessions(&self) -> usize {
        self.resources.active_sessions().await
    }

    pub async fn update_ice_servers(&self, configs: Vec<IceServerConfig>) {
        let mut guard = self.ice_servers.write().await;
        *guard = configs
            .into_iter()
            .map(|config| IceServer::from(&config))
            .collect();
    }

    fn is_client_supported(&self, client: &ClientMetadata) -> bool {
        match client.client_type {
            ClientType::Browser => self
                .config
                .system
                .is_browser_supported(&client.name, &client.version),
            ClientType::Mobile => self
                .config
                .system
                .is_mobile_supported(&client.name, &client.version),
        }
    }

    async fn current_ice_servers(&self) -> Vec<IceServer> {
        self.ice_servers.read().await.clone()
    }
}

impl SignalingService<NoopTokenValidator> {
    pub fn with_default_validator(config: Arc<ConfigSet>) -> Self {
        Self::new(config, NoopTokenValidator::default())
    }
}

impl From<&IceServerConfig> for IceServer {
    fn from(value: &IceServerConfig) -> Self {
        Self {
            urls: value.urls.clone(),
            username: value.username.clone().filter(|s| !s.is_empty()),
            credential: value.credential.clone().filter(|s| !s.is_empty()),
        }
    }
}

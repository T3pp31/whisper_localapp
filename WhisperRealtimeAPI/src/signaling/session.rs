use std::time::Instant;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientType {
    Browser,
    Mobile,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientMetadata {
    pub client_type: ClientType,
    pub name: String,
    pub version: String,
}

impl ClientMetadata {
    pub fn browser(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            client_type: ClientType::Browser,
            name: name.into(),
            version: version.into(),
        }
    }

    pub fn mobile(os: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            client_type: ClientType::Mobile,
            name: os.into(),
            version: version.into(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRequest {
    pub client: ClientMetadata,
    pub auth_token: String,
    pub retry: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IceServer {
    pub urls: Vec<String>,
    pub username: Option<String>,
    pub credential: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionResponse {
    pub session_id: String,
    pub ice_servers: Vec<IceServer>,
    pub max_bitrate_kbps: u32,
}

#[derive(Debug, Clone)]
pub struct SessionHandle {
    pub id: String,
    pub client: ClientMetadata,
    pub owner: String,
    pub created_at: Instant,
}

impl SessionHandle {
    pub fn new(id: String, client: ClientMetadata, owner: String) -> Self {
        Self {
            id,
            client,
            owner,
            created_at: Instant::now(),
        }
    }
}

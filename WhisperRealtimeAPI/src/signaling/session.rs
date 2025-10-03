//! シグナリングセッションおよびクライアント情報の型
use std::time::Instant;

/// クライアント種別
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClientType {
    Browser,
    Mobile,
}

/// クライアントの識別情報
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientMetadata {
    pub client_type: ClientType,
    pub name: String,
    pub version: String,
}

impl ClientMetadata {
    /// ブラウザクライアント情報を生成
    pub fn browser(name: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            client_type: ClientType::Browser,
            name: name.into(),
            version: version.into(),
        }
    }

    /// モバイルクライアント情報を生成
    pub fn mobile(os: impl Into<String>, version: impl Into<String>) -> Self {
        Self {
            client_type: ClientType::Mobile,
            name: os.into(),
            version: version.into(),
        }
    }
}

/// セッション開始要求
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRequest {
    pub client: ClientMetadata,
    pub auth_token: String,
    pub retry: bool,
}

/// ICEサーバ情報
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IceServer {
    pub urls: Vec<String>,
    pub username: Option<String>,
    pub credential: Option<String>,
}

/// セッション応答内容
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionResponse {
    pub session_id: String,
    pub ice_servers: Vec<IceServer>,
    pub max_bitrate_kbps: u32,
}

/// 有効なセッションのハンドル
#[derive(Debug, Clone)]
pub struct SessionHandle {
    pub id: String,
    pub client: ClientMetadata,
    pub owner: String,
    pub created_at: Instant,
}

impl SessionHandle {
    /// 生成時刻は現在時刻で初期化
    pub fn new(id: String, client: ClientMetadata, owner: String) -> Self {
        Self {
            id,
            client,
            owner,
            created_at: Instant::now(),
        }
    }
}

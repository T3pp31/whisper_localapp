use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    /// WebSocketシグナリングサーバのバインドアドレス（例: 127.0.0.1:8080）
    pub ws_bind_addr: String,
}


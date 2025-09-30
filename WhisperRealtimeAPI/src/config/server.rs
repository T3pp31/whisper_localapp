use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    /// WebSocketシグナリングサーバのバインドアドレス（例: 127.0.0.1:8080）
    pub ws_bind_addr: String,
    /// ASR gRPCサーバのバインドアドレス（例: 127.0.0.1:50051）
    pub asr_grpc_bind_addr: String,
}

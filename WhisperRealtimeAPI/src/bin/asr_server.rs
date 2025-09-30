use std::net::SocketAddr;

use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;
use tracing::{error, info};

use std::sync::Arc;
use whisper_realtime_api::asr::server::{into_server_service, LocalAsrService};
use whisper_realtime_api::asr::whisper_engine::WhisperEngine;
use whisper_realtime_api::config::ConfigSet;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let cfg = match ConfigSet::load_from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("failed to load config: {e}");
            std::process::exit(1);
        }
    };
    let bind = cfg.server.asr_grpc_bind_addr.parse::<SocketAddr>().expect("invalid asr_grpc_bind_addr");
    info!(addr = %bind, "starting ASR gRPC server");

    let listener = tokio::net::TcpListener::bind(bind).await.expect("bind");
    let incoming = TcpListenerStream::new(listener);

    let svc = if let Ok(engine) = WhisperEngine::load(cfg.whisper.clone()) {
        into_server_service(LocalAsrService::with_engine(Arc::new(engine)))
    } else {
        into_server_service(LocalAsrService::default())
    };
    if let Err(e) = Server::builder().add_service(svc).serve_with_incoming(incoming).await {
        error!(error = %e, "server error");
        std::process::exit(1);
    }
}

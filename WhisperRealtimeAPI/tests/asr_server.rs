use std::net::SocketAddr;
use std::sync::Arc;

use tokio_stream::wrappers::TcpListenerStream;
use tonic::transport::Server;

use whisper_realtime_api::asr::grpc_client::{asr_proto, GrpcAsrClient};
use whisper_realtime_api::asr::server::{into_server_service, LocalAsrService};
use whisper_realtime_api::config::ConfigSet;

#[tokio::test]
#[ignore]
async fn run_local_asr_server_and_client() {
    // サーバ起動（エフェメラルポート）
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let incoming = TcpListenerStream::new(listener);

    tokio::spawn(async move {
        let svc = into_server_service(LocalAsrService::default());
        let _ = Server::builder().add_service(svc).serve_with_incoming(incoming).await;
    });

    // クライアントで接続
    let cfg = ConfigSet::load_from_dir("config").unwrap();
    let asr_cfg = Arc::new(cfg.asr.clone());
    let endpoint = format!("http://{}", addr);
    let mut client = GrpcAsrClient::new(endpoint, asr_cfg.clone(), 16000, 1);

    // ストリーミング開始
    use tokio::sync::mpsc;
    let (audio_tx, audio_rx) = mpsc::channel::<bytes::Bytes>(4);
    let mut rx = client.start_streaming(audio_rx).await.expect("start");

    // 設定送付はクライアント内部が実施済み
    // 音声ダミーデータ送信
    let _ = audio_tx.send(bytes::Bytes::from_static(&[1,2,3,4])).await;
    drop(audio_tx);

    // レスポンス受信
    let mut got_final = false;
    for _ in 0..10 {
        if let Some(resp) = rx.recv().await { // mpsc::Receiver of responses in GrpcAsrClient
            if resp.is_final { got_final = true; break; }
        }
    }
    assert!(got_final, "should get final response");
}

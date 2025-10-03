use std::sync::Arc;

use hyper::{Body, Client, Method, Request};
use tokio::net::TcpListener;

use whisper_realtime_api::asr::{AsrManager, MockAsrClient};
use whisper_realtime_api::config::ConfigSet;
use whisper_realtime_api::http_api;

#[tokio::test]
async fn http_endpoints_basic() {
    let cfg = ConfigSet::load_from_env().expect("load config");

    // Mock ASR でHTTPサーバを起動
    let asr_cfg = Arc::new(cfg.asr.clone());
    let manager = Arc::new(AsrManager::new(MockAsrClient::new(asr_cfg.clone()), asr_cfg));
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().unwrap();
    let audio_cfg = cfg.audio.clone();

    tokio::spawn(async move {
        let _ = http_api::serve_http_with_listener::<MockAsrClient>(listener, manager, audio_cfg).await;
    });

    let client = Client::new();
    let base = format!("http://{}", addr);
    let session = "sess-http-1";

    // events 接続（200, content-type: text/event-stream）
    let req = Request::builder()
        .method(Method::GET)
        .uri(format!("{}/http/v1/sessions/{}/events", base, session))
        .body(Body::empty())
        .unwrap();
    let resp = client.request(req).await.expect("events resp");
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.headers().get("content-type").unwrap(), "text/event-stream");

    // chunk 投入（204）
    let input_sr = cfg.audio.input.sample_rate_hz as usize;
    let channels = cfg.audio.input.channels as usize;
    let frame_ms = cfg.audio.frame_assembler.frame_duration_ms as usize;
    let samples_per_frame = input_sr * frame_ms / 1000;
    let interleaved = vec![0_i16; samples_per_frame * channels];
    let mut bytes = Vec::with_capacity(interleaved.len() * 2);
    for s in &interleaved {
        bytes.extend_from_slice(&s.to_le_bytes());
    }
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("{}/http/v1/sessions/{}/chunk", base, session))
        .header("content-type", "application/octet-stream")
        .body(Body::from(bytes.clone()))
        .unwrap();
    let resp = client.request(req).await.expect("chunk resp");
    assert_eq!(resp.status(), 204);

    // finish（204）
    let req = Request::builder()
        .method(Method::POST)
        .uri(format!("{}/http/v1/sessions/{}/finish", base, session))
        .body(Body::empty())
        .unwrap();
    let resp = client.request(req).await.expect("finish resp");
    assert_eq!(resp.status(), 204);
}


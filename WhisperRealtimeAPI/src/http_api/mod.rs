//! HTTP インジェスト + SSE イベント配信
//!
//! エンドポイント:
//! - `POST /http/v1/sessions/:id/chunk`  生PCM(S16LE, little-endian)を受け取りバッファへ追加
//! - `POST /http/v1/sessions/:id/finish` セッションを終了しASRへフラッシュ
//! - `GET  /http/v1/sessions/:id/events`  SSEで partial/final イベントを送信
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use futures_util::future::BoxFuture;
use futures_util::FutureExt;
use hyper::body::to_bytes;
use hyper::service::Service;
use hyper::{Body, Method, Request, Response, Server, StatusCode};
use tokio::net::TcpListener;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use tracing::{error, info};

use crate::asr::{AsrManager, StreamingAsrClient, TranscriptUpdate};
use crate::config::AudioProcessingConfig;
use crate::ingest::PcmIngestor;

struct App<C>
where
    C: StreamingAsrClient + Send + Sync + 'static,
{
    ingestor: Arc<PcmIngestor<C>>,
}

impl<C> App<C>
where
    C: StreamingAsrClient + Send + Sync + 'static,
{
    fn new(ingestor: Arc<PcmIngestor<C>>) -> Self {
        Self { ingestor }
    }
}
impl<C> Clone for App<C>
where
    C: StreamingAsrClient + Send + Sync + 'static,
{
    fn clone(&self) -> Self {
        Self { ingestor: self.ingestor.clone() }
    }
}

impl<C> Service<Request<Body>> for App<C>
where
    C: StreamingAsrClient + Send + Sync + 'static,
{
    type Response = Response<Body>;
    type Error = Infallible;
    type Future = BoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(&mut self, _cx: &mut std::task::Context<'_>) -> std::task::Poll<Result<(), Self::Error>> {
        std::task::Poll::Ready(Ok(()))
    }

    fn call(&mut self, req: Request<Body>) -> Self::Future {
        let ingestor = self.ingestor.clone();
        async move { Ok(route(ingestor, req).await) }.boxed()
    }
}

/// ルーティング（簡易実装）: 上記エンドポイントへ分配
async fn route<C>(ingestor: Arc<PcmIngestor<C>>, req: Request<Body>) -> Response<Body>
where
    C: StreamingAsrClient + Send + Sync + 'static,
{
    let method = req.method().clone();
    let path = req.uri().path().to_string();

    // match paths
    // POST /http/v1/sessions/:id/chunk
    // POST /http/v1/sessions/:id/finish
    // GET  /http/v1/sessions/:id/events
    let prefix = "/http/v1/sessions/";
    if let Some(rest) = path.strip_prefix(prefix) {
        let mut parts = rest.splitn(2, '/');
        if let Some(session_id) = parts.next() {
            let tail = parts.next().unwrap_or("");
            match (method, tail) {
                (Method::POST, "chunk") => return handle_chunk(ingestor, session_id, req).await,
                (Method::POST, "finish") => return handle_finish(ingestor, session_id).await,
                (Method::GET, "events") => return handle_sse(ingestor, session_id).await,
                _ => {}
            }
        }
    }

    Response::builder()
        .status(StatusCode::NOT_FOUND)
        .body(Body::from("not found"))
        .unwrap()
}

/// PCMチャンクを受け取り、i16配列へ展開してインジェスタに転送
async fn handle_chunk<C>(
    ingestor: Arc<PcmIngestor<C>>,
    session_id: &str,
    req: Request<Body>,
)-> Response<Body>
where
    C: StreamingAsrClient + Send + Sync + 'static,
{
    if let Err(e) = ingestor.start_session(session_id).await {
        error!(error = %e, "start_session failed");
        return Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::from("start_session failed"))
            .unwrap();
    }

    let body_bytes = match to_bytes(req.into_body()).await {
        Ok(b) => b,
        Err(_) => {
            return Response::builder()
                .status(StatusCode::BAD_REQUEST)
                .body(Body::from("invalid body"))
                .unwrap()
        }
    };

    // bytes -> i16 LE
    let mut samples = Vec::with_capacity(body_bytes.len() / 2);
    let buf = body_bytes.as_ref();
    for chunk in buf.chunks_exact(2) {
        let v = i16::from_le_bytes([chunk[0], chunk[1]]);
        samples.push(v);
    }

    match ingestor.ingest_chunk(session_id, &samples).await {
        Ok(()) => Response::builder().status(StatusCode::NO_CONTENT).body(Body::empty()).unwrap(),
        Err(e) => {
            error!(error = %e, "ingest_chunk failed");
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from("ingest failed"))
                .unwrap()
        }
    }
}

/// セッションをフラッシュして終了
async fn handle_finish<C>(
    ingestor: Arc<PcmIngestor<C>>,
    session_id: &str,
)-> Response<Body>
where
    C: StreamingAsrClient + Send + Sync + 'static,
{
    match ingestor.finish_session(session_id).await {
        Ok(()) => Response::builder().status(StatusCode::NO_CONTENT).body(Body::empty()).unwrap(),
        Err(e) => {
            error!(error = %e, "finish_session failed");
            Response::builder()
                .status(StatusCode::INTERNAL_SERVER_ERROR)
                .body(Body::from("finish failed"))
                .unwrap()
        }
    }
}

/// SSEで partial/final のテキスト更新を逐次送出
async fn handle_sse<C>(ingestor: Arc<PcmIngestor<C>>, session_id: &str) -> Response<Body>
where
    C: StreamingAsrClient + Send + Sync + 'static,
{
    // channel for this connection
    let (tx, rx) = mpsc::channel::<Bytes>(32);
    let session = session_id.to_string();
    let asr = ingestor.asr_manager();

    tokio::spawn(async move {
        let mut event_id: u64 = 0;
        loop {
            match asr.poll_update(&session).await {
                Ok(Some(TranscriptUpdate::Partial { text, confidence })) => {
                    event_id += 1;
                    let payload = serde_json::json!({ "text": text, "confidence": confidence }).to_string();
                    let msg = format!("id: {}\nevent: partial\ndata: {}\n\n", event_id, payload);
                    if tx.send(Bytes::from(msg)).await.is_err() {
                        break;
                    }
                }
                Ok(Some(TranscriptUpdate::Final { text })) => {
                    event_id += 1;
                    let payload = serde_json::json!({ "text": text }).to_string();
                    let msg = format!("id: {}\nevent: final\ndata: {}\n\n", event_id, payload);
                    let _ = tx.send(Bytes::from(msg)).await;
                    // セッションを明示的にクリーンアップ
                    let _ = asr.drop_session(&session).await;
                    break;
                }
                Ok(None) => {
                    // session may not be ready yet; wait a bit
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                Err(_) => {
                    // wait for session creation
                    tokio::time::sleep(Duration::from_millis(100)).await;
                }
            }
        }
    });

    let stream = ReceiverStream::new(rx).map(Ok::<Bytes, Infallible>);
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "text/event-stream")
        .header("cache-control", "no-cache")
        .header("connection", "keep-alive")
        .body(Body::wrap_stream(stream))
        .unwrap()
}

/// 指定アドレスへHTTPサーバをバインドして起動
pub async fn serve_http<C>(
    bind_addr: &str,
    asr: Arc<AsrManager<C>>,
    audio_cfg: AudioProcessingConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    C: StreamingAsrClient + Send + Sync + 'static,
{
    let ingestor = Arc::new(PcmIngestor::new(asr, audio_cfg));
    let app = App::<C>::new(ingestor);
    let make_svc = hyper::service::make_service_fn(move |_| {
        let app = app.clone();
        async move { Ok::<_, Infallible>(app) }
    });
    let addr: SocketAddr = bind_addr.parse()?;
    info!(%addr, "HTTP ingest server listening");
    Server::bind(&addr).serve(make_svc).await?;
    Ok(())
}

/// 既存の `TcpListener` を用いてHTTPサーバを起動
pub async fn serve_http_with_listener<C>(
    listener: TcpListener,
    asr: Arc<AsrManager<C>>,
    audio_cfg: AudioProcessingConfig,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>>
where
    C: StreamingAsrClient + Send + Sync + 'static,
{
    let ingestor = Arc::new(PcmIngestor::new(asr, audio_cfg));
    let app = App::<C>::new(ingestor);
    let make_svc = hyper::service::make_service_fn(move |_| {
        let app = app.clone();
        async move { Ok::<_, Infallible>(app) }
    });
    let local = listener.local_addr()?;
    info!(%local, "HTTP ingest server listening");
    Server::from_tcp(listener.into_std()?)?.serve(make_svc).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn http_module_compiles() {
        assert!(true);
    }
}

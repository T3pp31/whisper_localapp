//! ローカルASR gRPCサーバ実装
//!
//! whisperエンジンが読み込める場合はそれを用い、読み込めない場合はモック応答を返します。
//! クライアントからは `Config` メッセージ→複数 `AudioContent` → ストリーム終了 の順で
//! 送信されることを想定しています。
use tokio_stream::StreamExt;
use tonic::{Request, Response, Status};

use crate::asr::grpc_client::asr_proto::asr_service_server::{AsrService, AsrServiceServer};
use crate::asr::grpc_client::asr_proto::{RecognizeConfig, SpeechRecognitionResult, StreamingRecognizeRequest, StreamingRecognizeResponse};
use crate::asr::whisper_engine::WhisperEngine;
use std::sync::Arc;
use tracing::{error, info, warn};

pub fn into_server_service<T: AsrService>(svc: T) -> AsrServiceServer<T> {
    AsrServiceServer::new(svc)
}

#[derive(Debug, Clone)]
pub struct LocalAsrService {
    engine: Option<Arc<WhisperEngine>>, // None の場合はモック応答
}

impl Default for LocalAsrService {
    fn default() -> Self {
        Self { engine: None }
    }
}

impl LocalAsrService {
    pub fn with_engine(engine: Arc<WhisperEngine>) -> Self {
        Self { engine: Some(engine) }
    }
}

#[tonic::async_trait]
impl AsrService for LocalAsrService {
    type StreamingRecognizeStream = tokio_stream::wrappers::ReceiverStream<Result<StreamingRecognizeResponse, Status>>;

    async fn streaming_recognize(
        &self,
        request: Request<tonic::Streaming<StreamingRecognizeRequest>>,
    ) -> Result<Response<Self::StreamingRecognizeStream>, Status> {
        let mut in_stream = request.into_inner();
        let (tx, rx) = tokio::sync::mpsc::channel::<Result<StreamingRecognizeResponse, Status>>(32);

        let engine = self.engine.clone();
        tokio::spawn(async move {
            // 最初のメッセージは設定
            let mut cfg: Option<RecognizeConfig> = None;
            let mut chunks: usize = 0;
            let mut buffer_i16: Vec<i16> = Vec::new();

            while let Some(msg) = in_stream.next().await {
                match msg {
                    Ok(StreamingRecognizeRequest { request }) => {
                        if let Some(req) = request {
                            match req {
                                crate::asr::grpc_client::asr_proto::streaming_recognize_request::Request::Config(c) => {
                                    cfg = Some(c);
                                    info!("ASR config received");
                                }
                                crate::asr::grpc_client::asr_proto::streaming_recognize_request::Request::AudioContent(bytes) => {
                                    chunks += 1;
                                    // s16le (little-endian) を i16 ストレージへ追記
                                    for ch in bytes.chunks_exact(2) {
                                        let v = i16::from_le_bytes([ch[0], ch[1]]);
                                        buffer_i16.push(v);
                                    }
                                    // 部分結果（任意・軽量）
                                    let partial = StreamingRecognizeResponse {
                                        results: vec![SpeechRecognitionResult {
                                            transcript: format!("partial chunk {} ({} bytes)", chunks, bytes.len()),
                                            confidence: 0.5,
                                            start_time: 0.0,
                                            end_time: 0.0,
                                        }],
                                        is_final: false,
                                    };
                                    if tx.send(Ok(partial)).await.is_err() { break; }
                                }
                            }
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "stream receive error");
                        let _ = tx.send(Err(Status::internal("receive error"))).await;
                        break;
                    }
                }
            }

            // 終了時に最終結果を送信（whisper利用 または モック）
            let (sr, ch) = cfg
                .as_ref()
                .map(|c| (c.sample_rate, c.channels))
                .unwrap_or((16000, 1));

            if let Some(engine) = &engine {
                match engine.transcribe_i16(&buffer_i16, sr, ch) {
                    Ok(text) => {
                        let final_resp = StreamingRecognizeResponse {
                            results: vec![SpeechRecognitionResult {
                                transcript: text,
                                confidence: 0.9,
                                start_time: 0.0,
                                end_time: 0.0,
                            }],
                            is_final: true,
                        };
                        let _ = tx.send(Ok(final_resp)).await;
                    }
                    Err(e) => {
                        let _ = tx.send(Err(Status::internal(format!("whisper error: {e}")))).await;
                    }
                }
            } else {
                let lang = cfg.as_ref().map(|c| c.language.clone()).unwrap_or_else(|| "auto".to_string());
                let final_resp = StreamingRecognizeResponse {
                    results: vec![SpeechRecognitionResult {
                        transcript: format!("final ({} chunks) [lang:{}]", chunks, lang),
                        confidence: 0.9,
                        start_time: 0.0,
                        end_time: 0.0,
                    }],
                    is_final: true,
                };
                let _ = tx.send(Ok(final_resp)).await;
            }
        });

        Ok(Response::new(tokio_stream::wrappers::ReceiverStream::new(rx)))
    }
}

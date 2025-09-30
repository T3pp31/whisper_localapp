use bytes::Bytes;
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tokio_stream::StreamExt;
use tonic::transport::Channel;
use tracing::{error, info, warn};

use crate::asr::AsrError;
use crate::config::AsrPipelineConfig;

// 生成されたgRPCコード
pub mod asr_proto {
    tonic::include_proto!("asr");
}

use asr_proto::asr_service_client::AsrServiceClient;
use asr_proto::{RecognizeConfig, StreamingRecognizeRequest, StreamingRecognizeResponse};

/// gRPCベースのASRクライアント
pub struct GrpcAsrClient {
    endpoint: String,
    config: Arc<AsrPipelineConfig>,
    sample_rate_hz: i32,
    channels: i32,
    client: Option<AsrServiceClient<Channel>>,
}

impl GrpcAsrClient {
    pub fn new(endpoint: String, config: Arc<AsrPipelineConfig>, sample_rate_hz: i32, channels: i32) -> Self {
        Self {
            endpoint,
            config,
            sample_rate_hz,
            channels,
            client: None,
        }
    }

    pub fn sample_rate(&self) -> i32 {
        self.sample_rate_hz
    }

    pub fn channels(&self) -> i32 {
        self.channels
    }

    /// gRPCクライアント接続
    async fn connect(&mut self) -> Result<(), AsrError> {
        let client = AsrServiceClient::connect(self.endpoint.clone())
            .await
            .map_err(|e| AsrError::Processing { message: format!("gRPC接続失敗: {}", e) })?;

        info!(endpoint = %self.endpoint, "ASR gRPCクライアント接続完了");
        self.client = Some(client);
        Ok(())
    }

    /// ストリーミング文字起こしを開始
    pub async fn start_streaming(
        &mut self,
        audio_rx: mpsc::Receiver<Bytes>,
    ) -> Result<mpsc::Receiver<StreamingRecognizeResponse>, AsrError> {
        if self.client.is_none() {
            self.connect().await?;
        }

        let client = self
            .client
            .as_mut()
            .ok_or_else(|| AsrError::Processing { message: "クライアント未接続".to_string() })?;

        let (request_tx, request_rx) = mpsc::channel::<StreamingRecognizeRequest>(100);
        let (response_tx, response_rx) = mpsc::channel::<StreamingRecognizeResponse>(100);

        // 最初に設定を送信
        let config = RecognizeConfig {
            language: self.config.model.language.clone(),
            sample_rate: self.sample_rate_hz,
            channels: self.channels,
        };

        request_tx
            .send(StreamingRecognizeRequest {
                request: Some(asr_proto::streaming_recognize_request::Request::Config(
                    config,
                )),
            })
            .await
            .map_err(|e| AsrError::Processing { message: format!("設定送信失敗: {}", e) })?;

        // 音声データ送信タスク
        tokio::spawn(async move {
            let mut audio_rx = audio_rx;
            while let Some(audio_data) = audio_rx.recv().await {
                let request = StreamingRecognizeRequest {
                    request: Some(asr_proto::streaming_recognize_request::Request::AudioContent(
                        audio_data.to_vec(),
                    )),
                };

                if request_tx.send(request).await.is_err() {
                    warn!("音声データ送信チャネル閉鎖");
                    break;
                }
            }
        });

        // gRPCストリーミング開始
        let request_stream = ReceiverStream::new(request_rx);
        let mut response_stream = client
            .streaming_recognize(request_stream)
            .await
            .map_err(|e| AsrError::Processing { message: format!("ストリーミング開始失敗: {}", e) })?
            .into_inner();

        // レスポンス受信タスク
        tokio::spawn(async move {
            while let Some(result) = response_stream.next().await {
                match result {
                    Ok(response) => {
                        if response_tx.send(response).await.is_err() {
                            warn!("レスポンス送信チャネル閉鎖");
                            break;
                        }
                    }
                    Err(e) => {
                        error!(error = %e, "レスポンス受信エラー");
                        break;
                    }
                }
            }
        });

        Ok(response_rx)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_grpc_client_creation() {
        let config = Arc::new(AsrPipelineConfig {
            service: crate::config::ServiceConfig {
                endpoint: "http://localhost:50051".to_string(),
                request_timeout_ms: 1500,
                max_stream_duration_s: 3600,
            },
            streaming: crate::config::StreamingConfig {
                partial_result_interval_ms: 200,
                finalization_silence_ms: 800,
                max_pending_requests: 4,
            },
            model: crate::config::ModelConfig {
                name: "base".to_string(),
                language: "ja".to_string(),
                enable_vad: true,
            },
        });

        let client = GrpcAsrClient::new(
            "http://localhost:50051".to_string(),
            config,
            16000,
            1,
        );
        assert!(client.client.is_none());
    }
}

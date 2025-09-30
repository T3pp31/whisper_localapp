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
    client: Option<AsrServiceClient<Channel>>,
}

impl GrpcAsrClient {
    pub fn new(endpoint: String, config: Arc<AsrPipelineConfig>) -> Self {
        Self {
            endpoint,
            config,
            client: None,
        }
    }

    /// gRPCクライアント接続
    async fn connect(&mut self) -> Result<(), AsrError> {
        let client = AsrServiceClient::connect(self.endpoint.clone())
            .await
            .map_err(|e| AsrError::Internal(format!("gRPC接続失敗: {}", e)))?;

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
            .ok_or_else(|| AsrError::Internal("クライアント未接続".to_string()))?;

        let (request_tx, request_rx) = mpsc::channel::<StreamingRecognizeRequest>(100);
        let (response_tx, response_rx) = mpsc::channel::<StreamingRecognizeResponse>(100);

        // 最初に設定を送信
        let config = RecognizeConfig {
            language: self.config.language.clone().unwrap_or_default(),
            sample_rate: self.config.input_sample_rate_hz as i32,
            channels: self.config.channels as i32,
        };

        request_tx
            .send(StreamingRecognizeRequest {
                request: Some(asr_proto::streaming_recognize_request::Request::Config(
                    config,
                )),
            })
            .await
            .map_err(|e| AsrError::Internal(format!("設定送信失敗: {}", e)))?;

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
            .map_err(|e| AsrError::Internal(format!("ストリーミング開始失敗: {}", e)))?
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
            model_name: "base".to_string(),
            language: Some("ja".to_string()),
            input_sample_rate_hz: 16000,
            channels: 1,
            compute_type: "float32".to_string(),
            beam_size: 5,
            vad_filter: true,
        });

        let client = GrpcAsrClient::new("http://localhost:50051".to_string(), config);
        assert!(client.client.is_none());
    }
}
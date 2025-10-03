//! ASR(自動音声認識) 管理モジュール
//!
//! `AsrManager` は `StreamingAsrClient` 実装（gRPCクライアントやモック）を保持し、
//! セッションの開始/音声フレーム送信/終了/更新取得をスレッドセーフに仲介します。
//!
//! - セッションは `RwLock<HashMap<..>>` により管理
//! - 各セッションは `Mutex<StreamingSession>` で直列化
//! - HTTPハンドラやインジェスタから非同期に利用されます
mod client;
mod error;
pub mod grpc_client;
pub mod server;
pub mod whisper_engine;
mod mock;

use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{Mutex, RwLock};

use crate::config::AsrPipelineConfig;

pub use client::{StreamingAsrClient, StreamingSession, TranscriptUpdate};
pub use error::AsrError;
pub use grpc_client::GrpcAsrClient;
pub use mock::MockAsrClient;

pub struct AsrManager<C>
where
    C: StreamingAsrClient + Send + Sync + 'static,
{
    client: C,
    sessions: RwLock<HashMap<String, Arc<Mutex<StreamingSession>>>>,
    config: Arc<AsrPipelineConfig>,
}

impl<C> AsrManager<C>
where
    C: StreamingAsrClient + Send + Sync + 'static,
{
    /// ASRクライアントと設定を受け取り、マネージャを生成
    pub fn new(client: C, config: Arc<AsrPipelineConfig>) -> Self {
        Self {
            client,
            sessions: RwLock::new(HashMap::new()),
            config,
        }
    }

    /// 新しいセッションを開始し、内部マップに登録
    pub async fn start_session(&self, session_id: &str) -> Result<(), AsrError> {
        let session = self.client.start_session(session_id)?;
        let mut guard = self.sessions.write().await;
        guard.insert(session_id.to_string(), Arc::new(Mutex::new(session)));
        Ok(())
    }

    /// 音声フレーム（f32 PCM, モノラル）を対象セッションへ送信
    pub async fn send_audio(&self, session_id: &str, frame: Vec<f32>) -> Result<(), AsrError> {
        let sessions = self.sessions.read().await;
        let session = sessions
            .get(session_id)
            .ok_or_else(|| AsrError::StreamNotFound {
                session_id: session_id.to_string(),
            })?
            .clone();
        let guard = session.lock().await;
        guard.send_audio(frame).await
    }

    /// 対象セッションに終了を通知
    pub async fn finish_session(&self, session_id: &str) -> Result<(), AsrError> {
        let sessions = self.sessions.read().await;
        let session = sessions
            .get(session_id)
            .ok_or_else(|| AsrError::StreamNotFound {
                session_id: session_id.to_string(),
            })?
            .clone();
        let guard = session.lock().await;
        guard.finish().await
    }

    /// ASRからの途中/最終更新をポーリング
    pub async fn poll_update(
        &self,
        session_id: &str,
    ) -> Result<Option<TranscriptUpdate>, AsrError> {
        let sessions = self.sessions.read().await;
        let session = sessions
            .get(session_id)
            .ok_or_else(|| AsrError::StreamNotFound {
                session_id: session_id.to_string(),
            })?
            .clone();
        let mut guard = session.lock().await;
        Ok(guard.next_update().await)
    }

    /// 内部管理からセッションを破棄（SSE完了時等に使用）
    pub async fn drop_session(&self, session_id: &str) -> Result<(), AsrError> {
        let mut sessions = self.sessions.write().await;
        sessions
            .remove(session_id)
            .map(|_| ())
            .ok_or_else(|| AsrError::StreamNotFound {
                session_id: session_id.to_string(),
            })
    }

    /// 使用中のASR設定を取得（共有参照を複製）
    pub fn config(&self) -> Arc<AsrPipelineConfig> {
        self.config.clone()
    }
}

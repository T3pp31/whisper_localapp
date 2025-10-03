//! ASRクライアント共通の型とトレイト
//!
//! - `TranscriptUpdate` は途中/最終のテキスト更新イベント
//! - `StreamingSession` は1セッションの送受信チャネルを保持
//! - `StreamingAsrClient` はセッション開始を提供する最小インタフェース
use tokio::sync::mpsc;

use super::error::AsrError;

/// 文字起こし結果の更新イベント
#[derive(Debug, Clone, PartialEq)]
pub enum TranscriptUpdate {
    Partial { text: String, confidence: f32 },
    Final { text: String },
}

/// ストリーミングセッションのハンドル
#[derive(Debug)]
pub struct StreamingSession {
    session_id: String,
    command_tx: mpsc::Sender<AudioCommand>,
    update_rx: mpsc::Receiver<TranscriptUpdate>,
}

#[derive(Debug)]
pub(crate) enum AudioCommand {
    Frame(Vec<f32>),
    Finish,
}

impl StreamingSession {
    /// 内部用: セッションIDと送受信チャネルで初期化
    pub(crate) fn new(
        session_id: impl Into<String>,
        command_tx: mpsc::Sender<AudioCommand>,
        update_rx: mpsc::Receiver<TranscriptUpdate>,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            command_tx,
            update_rx,
        }
    }

    /// セッションIDを取得
    pub fn session_id(&self) -> &str {
        &self.session_id
    }

    /// 音声フレームを送信（f32モノラル、-1.0..1.0）
    pub async fn send_audio(&self, frame: Vec<f32>) -> Result<(), AsrError> {
        self.command_tx
            .send(AudioCommand::Frame(frame))
            .await
            .map_err(|_| AsrError::Processing {
                message: "audio command channel closed".to_string(),
            })
    }

    /// セッションの終了を送信
    pub async fn finish(&self) -> Result<(), AsrError> {
        self.command_tx
            .send(AudioCommand::Finish)
            .await
            .map_err(|_| AsrError::Processing {
                message: "audio command channel closed".to_string(),
            })
    }

    /// 次の更新イベントを待機
    pub async fn next_update(&mut self) -> Option<TranscriptUpdate> {
        self.update_rx.recv().await
    }
}

/// ASRクライアント最小インタフェース
pub trait StreamingAsrClient: Send + Sync {
    fn start_session(&self, session_id: &str) -> Result<StreamingSession, AsrError>;
}

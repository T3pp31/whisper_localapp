use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;

use crate::asr::{AsrManager, StreamingAsrClient};
use crate::audio_pipeline::AudioPipeline;
use crate::config::AudioProcessingConfig;

/// インジェスト処理で発生しうるエラー
#[derive(thiserror::Error, Debug)]
pub enum IngestError {
    #[error("session not found: {0}")]
    NotFound(String),
    #[error("asr error: {0}")]
    Asr(#[from] crate::asr::AsrError),
}

#[derive(Debug)]
struct SessionState {
    pipeline: AudioPipeline,
}

/// PCM(S16LE)チャンクを受け取り、フレーム化してASRへ送る簡易インジェスタ
///
/// - 入力は config.audio.input.* に従う（サンプルレート/チャネル）
/// - 内部でフレーム再構成・リサンプル・正規化を行い、ターゲットフレームをASRへ送出
pub struct PcmIngestor<C>
where
    C: StreamingAsrClient + Send + Sync + 'static,
{
    asr: Arc<AsrManager<C>>,
    audio_cfg: AudioProcessingConfig,
    sessions: Mutex<HashMap<String, SessionState>>,
}

impl<C> PcmIngestor<C>
where
    C: StreamingAsrClient + Send + Sync + 'static,
{
    /// ASRマネージャと音声設定からインジェスタを作成
    pub fn new(asr: Arc<AsrManager<C>>, audio_cfg: AudioProcessingConfig) -> Self {
        Self {
            asr,
            audio_cfg,
            sessions: Mutex::new(HashMap::new()),
        }
    }

    pub fn asr_manager(&self) -> Arc<AsrManager<C>> {
        self.asr.clone()
    }

    /// セッション開始（既に存在すれば何もしない）
    pub async fn start_session(&self, session_id: &str) -> Result<(), IngestError> {
        let need_start = {
            let map = self.sessions.lock();
            !map.contains_key(session_id)
        };

        if need_start {
            // 外部I/Oをロック外で実行
            self.asr.start_session(session_id).await?;
            let mut map = self.sessions.lock();
            map.entry(session_id.to_string()).or_insert_with(|| SessionState {
                pipeline: AudioPipeline::new(self.audio_cfg.clone()),
            });
        }
        Ok(())
    }

    /// PCM(S16LE)のインターリーブ配列を投入
    /// - frame: S16LEの生バイト列を i16 に展開して渡すことを想定
    pub async fn ingest_chunk(&self, session_id: &str, samples_i16: &[i16]) -> Result<(), IngestError> {
        let frames = {
            let mut map = self.sessions.lock();
            let state = map
                .get_mut(session_id)
                .ok_or_else(|| IngestError::NotFound(session_id.to_string()))?;
            state.pipeline.process(samples_i16)
        };

        for f in frames {
            self.asr.send_audio(session_id, f).await?;
        }
        Ok(())
    }

    /// セッションをフラッシュして終了
    pub async fn finish_session(&self, session_id: &str) -> Result<(), IngestError> {
        let mut maybe_flush = None;
        {
            let mut map = self.sessions.lock();
            let state = map
                .get_mut(session_id)
                .ok_or_else(|| IngestError::NotFound(session_id.to_string()))?;
            maybe_flush = state.pipeline.flush();
        }

        if let Some(rem) = maybe_flush {
            self.asr.send_audio(session_id, rem).await?;
        }
        self.asr.finish_session(session_id).await?;

        // 最後にローカル状態を破棄
        let mut map = self.sessions.lock();
        map.remove(session_id);
        Ok(())
    }
}

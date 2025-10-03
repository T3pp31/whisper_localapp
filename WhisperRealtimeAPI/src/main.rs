//! バイナリエントリ: HTTP(SSE) インジェストサーバの起動
//!
//! - 環境変数（`WHISPER_REALTIME_CONFIG_DIR`）または `config/` から設定を読み込み
//! - ASR クライアントと音声パイプラインを初期化
//! - HTTPエンドポイントを起動し、PCMチャンクの受付とSSEでの途中結果/最終結果を提供
use std::sync::Arc;

use tracing::{error, info};
use tracing_subscriber::EnvFilter;

use whisper_realtime_api::asr::{AsrManager, GrpcAsrClient, grpc_client::GrpcAsrClientAdapter};
use whisper_realtime_api::audio_pipeline::AudioPipeline;
use whisper_realtime_api::config::ConfigSet;
use whisper_realtime_api::http_api;

#[tokio::main]
async fn main() {
    init_tracing();

    match ConfigSet::load_from_env() {
        Ok(config) => {
            let config = Arc::new(config);
            info!(root = ?config.root(), "configuration loaded");

            // HTTP(SSE)サーバのみを起動
            // gRPCクライアントはHTTPインジェスト→ASR間の橋渡しとして利用する
            let asr_config = Arc::new(config.asr.clone());
            let target_sr = config.audio.target.sample_rate_hz as i32;
            let target_ch = config.audio.target.channels as i32;
            let grpc_client = GrpcAsrClient::new(
                asr_config.service.endpoint.clone(),
                asr_config.clone(),
                target_sr,
                target_ch,
            );
            let asr_manager = Arc::new(AsrManager::new(GrpcAsrClientAdapter::from_client(grpc_client), asr_config));
            let mut audio_pipeline = AudioPipeline::new(config.audio.clone());

            info!(max_sessions = config.system.resources.max_concurrent_sessions, "http ingest initialized");

            // 音声パイプラインを一度「空データ」でウォームアップして初回レイテンシを低減
            let input_frame_samples = (config.audio.input.sample_rate_hz as usize
                * config.audio.frame_assembler.frame_duration_ms as usize)
                / 1000;
            let silent_frame =
                vec![0_i16; input_frame_samples * config.audio.input.channels as usize];
            let frames = audio_pipeline.process(&silent_frame);
            info!(
                produced_frames = frames.len(),
                "audio pipeline warm-up complete"
            );

            // HTTP ingest + SSE server 起動
            let http_addr = config.server.http_bind_addr.clone();
            let http_asr = asr_manager.clone();
            let http_audio_cfg = config.audio.clone();
            let http_task = tokio::spawn(async move {
                if let Err(e) = http_api::serve_http::<GrpcAsrClientAdapter>(&http_addr, http_asr, http_audio_cfg).await {
                    error!(error = %e, "failed to start http ingest server");
                }
            });
            // 終了待機（HTTPサーバのみ）
            let _ = http_task.await;
        }
        Err(err) => {
            error!(error = ?err, "failed to load configuration");
            std::process::exit(1);
        }
    }
}

fn init_tracing() {
    // 環境変数 `RUST_LOG` などからログレベルを設定するトレース購読者を初期化
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .with_target(false)
        .finish();

    if let Err(err) = tracing::subscriber::set_global_default(subscriber) {
        eprintln!("failed to install tracing subscriber: {err}");
    }
}

mod audio;
mod config;
mod handlers;
mod models;
mod whisper;

use crate::config::Config;
use crate::handlers::{AppState, add_cors_headers};
use crate::whisper::WhisperEngine;
use axum::{
    routing::{get, post, options},
    Router,
};
use std::net::SocketAddr;
use tower::ServiceBuilder;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // ログの初期化
    env_logger::init();

    println!("WhisperBackendAPI を起動中...");

    // 設定ファイルの読み込み
    let config = Config::load_or_create_default("config.toml")?;

    // 設定の検証
    config.validate()?;

    println!("設定ファイルを読み込みました");
    println!("サーバーアドレス: {}", config.server_address());
    println!("Whisperモデル: {}", config.whisper.model_path);

    // アプリケーション状態の初期化
    let mut app_state = AppState::new(config.clone());

    // Whisperエンジンの初期化
    match WhisperEngine::new(&config.whisper.model_path, &config) {
        Ok(engine) => {
            println!("Whisperエンジンを初期化しました");
            app_state = app_state.with_whisper_engine(engine);
        }
        Err(e) => {
            eprintln!("Whisperエンジンの初期化に失敗しました: {}", e);
            eprintln!("サーバーは起動しますが、文字起こし機能は利用できません");
        }
    }

    // CORSレイヤーの設定
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    // ルーターの構築
    let app = Router::new()
        // 文字起こしエンドポイント
        .route("/transcribe", post(handlers::transcribe_basic))
        .route("/transcribe-with-timestamps", post(handlers::transcribe_with_timestamps))

        // 情報取得エンドポイント
        .route("/models", get(handlers::get_models))
        .route("/languages", get(handlers::get_languages))
        .route("/health", get(handlers::health_check))
        .route("/stats", get(handlers::get_stats))

        // CORS プリフライトリクエスト対応
        .route("/transcribe", options(add_cors_headers))
        .route("/transcribe-with-timestamps", options(add_cors_headers))
        .route("/models", options(add_cors_headers))
        .route("/languages", options(add_cors_headers))
        .route("/health", options(add_cors_headers))
        .route("/stats", options(add_cors_headers))

        // ミドルウェアの追加
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(cors)
        )

        // アプリケーション状態の共有
        .with_state(app_state);

    // サーバーアドレスの解析
    let addr: SocketAddr = config.server_address().parse()
        .map_err(|e| anyhow::anyhow!("無効なサーバーアドレス: {}", e))?;

    println!("サーバーを起動します: http://{}", addr);
    println!("API エンドポイント:");
    println!("  POST /transcribe - 基本的な文字起こし");
    println!("  POST /transcribe-with-timestamps - タイムスタンプ付き文字起こし");
    println!("  GET  /models - 利用可能なモデル一覧");
    println!("  GET  /languages - サポートされている言語一覧");
    println!("  GET  /health - ヘルスチェック");
    println!("  GET  /stats - サーバー統計情報");
    println!();
    println!("使用例:");
    println!("  curl -F \"file=@audio.wav\" http://{}/transcribe", addr);
    println!("  curl -F \"file=@audio.wav\" -F \"language=ja\" http://{}/transcribe-with-timestamps", addr);

    // サーバーの起動
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app)
        .await
        .map_err(|e| anyhow::anyhow!("サーバーの起動に失敗: {}", e))?;

    Ok(())
}

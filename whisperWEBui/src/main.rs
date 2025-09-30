use std::net::SocketAddr;
use whisper_webui::{config::Config, handlers::AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();

    println!("Whisper WebUI を起動中...");

    let config = Config::load_or_create_default("config.toml")?;
    config.validate()?;

    println!("設定ファイルを読み込みました");
    println!("WebUIサーバーアドレス: {}", config.server_address());
    println!("バックエンドAPI: {}", config.backend.base_url);
    if config.realtime.enabled {
        let dir = config.realtime.config_dir.as_deref().unwrap_or("(未設定)");
        println!("リアルタイムバックエンド: 有効 ({})", dir);
    } else {
        println!("リアルタイムバックエンド: 無効");
    }

    let app_state = AppState::new(config.clone());
    let app = whisper_webui::create_app(app_state);

    let addr: SocketAddr = config
        .server_address()
        .parse()
        .map_err(|e| anyhow::anyhow!("無効なサーバーアドレス: {}", e))?;

    println!("WebUIサーバーを起動します: http://{}", addr);
    println!("APIエンドポイント:");
    println!("  GET  / - メイン画面");
    println!("  POST /api/upload - ファイルアップロード");
    println!("  GET  /api/health - バックエンドのヘルスチェック");
    println!("  GET  /api/stats - バックエンドの統計情報");
    println!("  GET  /api/models - 利用可能なモデル一覧");
    println!("  GET  /api/languages - サポート言語一覧");
    println!("  GET  /api/gpu-status - GPU使用状態");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app)
        .await
        .map_err(|e| anyhow::anyhow!("WebUIサーバーの起動に失敗: {}", e))?;

    Ok(())
}

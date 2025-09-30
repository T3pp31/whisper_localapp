use std::fs;
use std::path::PathBuf;
use whisper_webui::config::Config;

#[test]
fn test_load_config_from_file_with_backend_8081() {
    // 一時ファイルパスを生成
    let mut path = std::env::temp_dir();
    let filename = format!("whisper_webui_config_{}.toml", uuid::Uuid::new_v4());
    path.push(filename);

    // 8081を指す設定ファイルを作成
    let toml = r#"
[server]
host = "127.0.0.1"
port = 3001
max_request_size_mb = 110

[backend]
base_url = "http://127.0.0.1:8081"
timeout_seconds = 300

[webui]
title = "Whisper WebUI"
max_file_size_mb = 100
allowed_extensions = ["wav", "mp3", "m4a", "flac", "ogg", "mp4", "mov", "avi", "mkv"]
default_language = "ja"
default_with_timestamps = true
timeline_update_interval_ms = 200
upload_prompt_text = "音声ファイルをドラッグ&ドロップするか、クリックして選択してください"
upload_success_text = "{filename} を選択しました"
stats_average_processing_time_label = "平均処理時間 (音声1分あたりの文字起こし所要時間)"
stats_average_processing_time_unit = "秒 / 音声1分"

[realtime]
enabled = true
config_dir = "../WhisperRealtimeAPI/config"
default_client_type = "browser"
default_client_name = "Chrome"
default_client_version = "130"
default_token_subject = "web-demo"
heartbeat_interval_ms = 30000
"#;

    fs::write(&path, toml).expect("設定ファイルの作成に失敗しました");

    // 読み込み検証
    let config = Config::load_or_create_default(&path).expect("設定ファイルの読み込みに失敗しました");
    assert_eq!(config.backend.base_url, "http://127.0.0.1:8081");
    assert_eq!(config.server.port, 3001);

    // 片付け（ベストエフォート）
    let _ = fs::remove_file(&path);
}


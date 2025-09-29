use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::Value;
use tower::ServiceExt;
use whisper_webui::{config::Config, handlers::AppState};

#[tokio::test]
async fn test_index_page() {
    let config = Config::default();
    let app_state = AppState::new(config);
    let app = whisper_webui::create_app(app_state);

    let request = Request::builder().uri("/").body(Body::empty()).unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_backend_health_endpoint() {
    let config = Config::default();
    let app_state = AppState::new(config);
    let app = whisper_webui::create_app(app_state);

    let request = Request::builder()
        .uri("/api/health")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert!(json.is_object());
}

#[tokio::test]
async fn test_backend_languages_endpoint() {
    let config = Config::default();
    let app_state = AppState::new(config);
    let app = whisper_webui::create_app(app_state);

    let request = Request::builder()
        .uri("/api/languages")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();
    assert!(json.is_object());
}

#[tokio::test]
async fn test_config_validation() {
    let mut config = Config::default();
    assert!(config.validate().is_ok());

    config.server.port = 0;
    assert!(config.validate().is_err());

    config.server.port = 3000;
    config.server.max_request_size_mb = 0;
    assert!(config.validate().is_err());

    config.server.max_request_size_mb = config.webui.max_file_size_mb.saturating_sub(1);
    assert!(config.validate().is_err());

    config.server.max_request_size_mb = config.webui.max_file_size_mb + 10;
    config.backend.base_url = String::new();
    assert!(config.validate().is_err());

    config.backend.base_url = "http://localhost:8000".to_string();
    config.webui.max_file_size_mb = 0;
    assert!(config.validate().is_err());

    config.webui.max_file_size_mb = 100;
    config.webui.timeline_update_interval_ms = 0;
    assert!(config.validate().is_err());

    config.webui.timeline_update_interval_ms = 200;
    config.webui.upload_prompt_text = String::new();
    assert!(config.validate().is_err());

    config.webui.upload_prompt_text = "案内".to_string();
    config.webui.upload_success_text = String::new();
    assert!(config.validate().is_err());

    config.webui.upload_success_text = "案内完了".to_string();
    config.webui.stats_average_processing_time_label = String::new();
    assert!(config.validate().is_err());

    config.webui.stats_average_processing_time_label = "平均処理時間 (1分音声あたり)".to_string();
    assert!(config.validate().is_ok());
}

#[tokio::test]
async fn test_file_extension_validation() {
    let config = Config::default();

    assert!(config.is_allowed_extension("wav"));
    assert!(config.is_allowed_extension("mp3"));
    assert!(config.is_allowed_extension("WAV"));
    assert!(config.is_allowed_extension("MP3"));
    assert!(!config.is_allowed_extension("txt"));
    assert!(!config.is_allowed_extension("doc"));
}

#[tokio::test]
async fn test_max_file_size_calculation() {
    let config = Config::default();
    let expected_bytes = config.webui.max_file_size_mb * 1024 * 1024;
    assert_eq!(config.max_file_size_bytes(), expected_bytes as usize);
}

#[tokio::test]
async fn test_max_request_size_calculation() {
    let config = Config::default();
    let expected_bytes = config.server.max_request_size_mb * 1024 * 1024;
    assert_eq!(config.max_request_size_bytes(), expected_bytes as usize);
}

#[tokio::test]
async fn test_server_address_format() {
    let config = Config::default();
    let expected_address = format!("{}:{}", config.server.host, config.server.port);
    assert_eq!(config.server_address(), expected_address);
}

#[tokio::test]
async fn test_index_contains_language_and_timeline_config() {
    let mut config = Config::default();
    config.webui.default_language = Some("ja".to_string());
    config.webui.timeline_update_interval_ms = 250;

    let app_state = AppState::new(config);
    let app = whisper_webui::create_app(app_state);

    let request = Request::builder().uri("/").body(Body::empty()).unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();

    assert!(html.contains("data-default-language=\"ja\""));
    assert!(html.contains("data-timeline-update-ms=\"250\""));
}

#[tokio::test]
async fn test_index_contains_transcribe_button() {
    let config = Config::default();
    let app_state = AppState::new(config);
    let app = whisper_webui::create_app(app_state);

    let request = Request::builder().uri("/").body(Body::empty()).unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();

    assert!(html.contains("id=\"transcribe-btn\""));
    assert!(html.contains("data-loading-label=\"文字起こし中...\""));
}

#[tokio::test]
async fn test_index_contains_upload_ui_configuration() {
    let mut config = Config::default();
    config.webui.upload_prompt_text = "ドラッグ&ドロップまたはクリックで選択".to_string();
    config.webui.upload_success_text = "✓ {filename} を準備しました".to_string();

    let app_state = AppState::new(config);
    let app = whisper_webui::create_app(app_state);

    let request = Request::builder().uri("/").body(Body::empty()).unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();

    assert!(html.contains("id=\"upload-text\""));
    assert!(html.contains("data-default-text=\"ドラッグ&amp;ドロップまたはクリックで選択\""));
    assert!(html.contains("data-success-text=\"✓ {filename} を準備しました\""));
    assert!(html.contains("id=\"upload-preview\""));
    assert!(html.contains("id=\"upload-audio-preview\""));
}

#[tokio::test]
async fn test_index_contains_stats_average_processing_config() {
    let mut config = Config::default();
    config.webui.stats_average_processing_time_label =
        "平均処理時間 (音声1分あたりの所要時間)".to_string();
    config.webui.stats_average_processing_time_unit = "秒 / 音声1分".to_string();

    let app_state = AppState::new(config);
    let app = whisper_webui::create_app(app_state);

    let request = Request::builder().uri("/").body(Body::empty()).unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();

    assert!(html.contains(
        "data-stats-average-processing-time-label=\"平均処理時間 (音声1分あたりの所要時間)\""
    ));
    assert!(html.contains(
        "data-stats-average-processing-time-unit=\"秒 / 音声1分\""
    ));
}

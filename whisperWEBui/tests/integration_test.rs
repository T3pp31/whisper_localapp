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

    let request = Request::builder()
        .uri("/")
        .body(Body::empty())
        .unwrap();

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
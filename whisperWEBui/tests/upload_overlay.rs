use axum::{
    body::Body,
    http::Request,
};
use tower::ServiceExt;
use whisper_webui::{config::Config, handlers::AppState};

#[tokio::test]
async fn index_uses_file_input_overlay_not_hidden() {
    let config = Config::default();
    let app_state = AppState::new(config);
    let app = whisper_webui::create_app(app_state);

    let request = Request::builder().uri("/").body(Body::empty()).unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert!(response.status().is_success());

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();
    eprintln!("HTML length: {}", html.len());
    eprintln!("Has upload-area: {}", html.contains("id=\"upload-area\""));
    eprintln!("Has upload-content: {}", html.contains("class=\"upload-content\""));
    eprintln!("Has upload-preview: {}", html.contains("id=\"upload-preview\""));
    eprintln!("Has upload-text: {}", html.contains("id=\"upload-text\""));
    eprintln!("Has file-input: {}", html.contains("id=\"file-input\""));

    // hidden属性を使わず、オーバーレイクラスを利用していること
    assert!(html.contains(r#"id="file-input""#));
    assert!(html.contains(r#"class="file-input-overlay""#));
    assert!(!html.contains(r#"id="file-input" hidden"#));
}

#[tokio::test]
async fn static_css_contains_overlay_class() {
    let config = Config::default();
    let app_state = AppState::new(config);
    let app = whisper_webui::create_app(app_state);

    let request = Request::builder()
        .uri("/static/css/style.css")
        .body(Body::empty())
        .unwrap();

    let response = app.oneshot(request).await.unwrap();
    assert!(response.status().is_success());

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let css = String::from_utf8(body.to_vec()).unwrap();

    assert!(css.contains(".file-input-overlay"));
}

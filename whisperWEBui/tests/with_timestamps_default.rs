use axum::{body::Body, http::Request};
use tower::ServiceExt;
use whisper_webui::config::Config;
use whisper_webui::handlers::AppState;

#[tokio::test]
async fn index_has_with_timestamps_checked_by_default() {
    let config = Config::default();
    assert!(config.webui.default_with_timestamps, "expected default_with_timestamps to be true");

    let app_state = AppState::new(config);
    let app = whisper_webui::create_app(app_state);

    let request = Request::builder().uri("/").body(Body::empty()).unwrap();
    let response = app.oneshot(request).await.unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();

    let pos = html.find("with-timestamps").expect("with-timestamps checkbox should exist");
    let end = html[pos..].find('>').map(|i| pos + i).unwrap_or(html.len());
    let tag = &html[pos..end];
    assert!(tag.contains("checked"), "with-timestamps checkbox should be checked by default");
}

#[tokio::test]
async fn index_has_with_timestamps_unchecked_when_configured() {
    let mut config = Config::default();
    config.webui.default_with_timestamps = false;

    let app_state = AppState::new(config);
    let app = whisper_webui::create_app(app_state);

    let request = Request::builder().uri("/").body(Body::empty()).unwrap();
    let response = app.oneshot(request).await.unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let html = String::from_utf8(body.to_vec()).unwrap();

    // Should render the checkbox without the 'checked' attribute
    let pos = html.find("with-timestamps").expect("with-timestamps checkbox should exist");
    let end = html[pos..].find('>').map(|i| pos + i).unwrap_or(html.len());
    let tag = &html[pos..end];
    assert!(
        !tag.contains("checked"),
        "with-timestamps checkbox should not be checked when disabled in config"
    );
}

pub mod client;
pub mod config;
pub mod handlers;

use crate::handlers::AppState;
use axum::{
    extract::DefaultBodyLimit,
    routing::{delete, get, post},
    Router,
};
use tower::ServiceBuilder;
use tower_http::{
    cors::{Any, CorsLayer},
    services::ServeDir,
    trace::TraceLayer,
};

pub fn create_app(app_state: AppState) -> Router {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let max_request_size = app_state.config.max_request_size_bytes();

    Router::new()
        .route("/", get(handlers::index))
        .route("/api/upload", post(handlers::upload_file))
        .route("/api/health", get(handlers::backend_health))
        .route("/api/stats", get(handlers::backend_stats))
        .route("/api/models", get(handlers::backend_models))
        .route("/api/languages", get(handlers::backend_languages))
        .route("/api/gpu-status", get(handlers::backend_gpu_status))
        .route("/api/realtime/config", get(handlers::realtime_config))
        .route(
            "/api/realtime/session",
            post(handlers::realtime_start_session),
        )
        .route(
            "/api/realtime/session/{id}/heartbeat",
            post(handlers::realtime_heartbeat),
        )
        .route(
            "/api/realtime/session/{id}",
            delete(handlers::realtime_end_session),
        )
        .route("/ws/realtime/:session_id", get(handlers::websocket_handler))
        .nest_service("/static", ServeDir::new("static"))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(cors)
                .layer(DefaultBodyLimit::max(max_request_size)),
        )
        .with_state(app_state)
}

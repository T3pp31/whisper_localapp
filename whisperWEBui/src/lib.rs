pub mod client;
pub mod config;
pub mod handlers;

use crate::handlers::AppState;
use axum::{
    routing::{get, post},
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

    Router::new()
        .route("/", get(handlers::index))
        .route("/api/upload", post(handlers::upload_file))
        .route("/api/health", get(handlers::backend_health))
        .route("/api/stats", get(handlers::backend_stats))
        .route("/api/models", get(handlers::backend_models))
        .route("/api/languages", get(handlers::backend_languages))
        .route("/api/gpu-status", get(handlers::backend_gpu_status))
        .nest_service("/static", ServeDir::new("static"))
        .layer(
            ServiceBuilder::new()
                .layer(TraceLayer::new_for_http())
                .layer(cors)
        )
        .with_state(app_state)
}
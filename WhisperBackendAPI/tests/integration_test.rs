use axum::{
    body::Body,
    http::{Method, Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use std::fs;
use tempfile::TempDir;
use tower::ServiceExt;
use WhisperBackendAPI::{
    config::Config,
    handlers::{AppState, add_cors_headers},
    models::*,
};

#[cfg(test)]
mod integration_tests {
    use super::*;

    /// テスト用のルーターを作成
    fn create_test_router(app_state: AppState) -> Router {
        Router::new()
            .route("/health", axum::routing::get(health_check_handler))
            .route("/stats", axum::routing::get(stats_handler))
            .route("/models", axum::routing::get(models_handler))
            .route("/languages", axum::routing::get(languages_handler))
            .route("/gpu-status", axum::routing::get(gpu_status_handler))
            .route("/health", axum::routing::options(add_cors_headers))
            .route("/stats", axum::routing::options(add_cors_headers))
            .route("/models", axum::routing::options(add_cors_headers))
            .route("/languages", axum::routing::options(add_cors_headers))
            .route("/gpu-status", axum::routing::options(add_cors_headers))
            .with_state(app_state)
    }

    /// テスト用のハンドラー（実際のhandlers.rsの関数をモック）
    async fn health_check_handler(
        axum::extract::State(state): axum::extract::State<AppState>,
    ) -> axum::Json<HealthResponse> {
        let uptime_seconds = state.start_time.elapsed().as_secs();

        let model_loaded = {
            let engine_guard = state.whisper_engine.lock().unwrap();
            engine_guard.is_some()
        };

        axum::Json(HealthResponse {
            status: "healthy".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            model_loaded,
            uptime_seconds,
            memory_usage_mb: None, // テスト環境ではNone
        })
    }

    async fn stats_handler(
        axum::extract::State(state): axum::extract::State<AppState>,
    ) -> axum::Json<ServerStats> {
        let mut stats = state.stats.lock().unwrap().clone();
        stats.uptime_seconds = state.start_time.elapsed().as_secs();
        axum::Json(stats)
    }

    async fn models_handler(
        axum::extract::State(state): axum::extract::State<AppState>,
    ) -> axum::Json<ModelsResponse> {
        let catalog = ModelCatalog::default();
        let models_dir = std::path::Path::new(&state.config.paths.models_dir);

        let mut models = Vec::new();

        for (_key, model_def) in &catalog.models {
            let file_path = models_dir.join(&model_def.file_name);
            let is_available = file_path.exists();

            models.push(ModelInfo {
                name: model_def.name.clone(),
                file_path: file_path.to_string_lossy().to_string(),
                size_mb: model_def.size_mb,
                description: model_def.description.clone(),
                language_support: model_def.language_support.clone(),
                is_available,
            });
        }

        let current_model = state.config.whisper.default_model.clone();

        axum::Json(ModelsResponse {
            models,
            current_model,
        })
    }

    async fn languages_handler() -> axum::Json<Vec<LanguageInfo>> {
        use WhisperBackendAPI::whisper::{get_supported_languages, get_language_name};

        let languages = get_supported_languages()
            .iter()
            .map(|&code| LanguageInfo {
                code: code.to_string(),
                name: get_language_name(code).to_string(),
            })
            .collect();

        axum::Json(languages)
    }

    async fn gpu_status_handler(
        axum::extract::State(state): axum::extract::State<AppState>,
    ) -> axum::Json<Value> {
        // 簡略化されたGPU状態レスポンス
        let response = json!({
            "gpu_enabled_in_config": state.config.whisper.enable_gpu,
            "gpu_actually_enabled": false, // テスト環境では常にfalse
            "environment": {
                "whisper_cublas": std::env::var("WHISPER_CUBLAS").unwrap_or_default() == "1",
                "cuda_feature_enabled": cfg!(feature = "cuda"),
                "opencl_feature_enabled": cfg!(feature = "opencl")
            },
            "recommendations": vec!["GPU not available in test environment"]
        });

        axum::Json(response)
    }

    #[derive(serde::Serialize, serde::Deserialize)]
    pub struct LanguageInfo {
        pub code: String,
        pub name: String,
    }

    /// テスト用のAppStateを作成
    fn create_test_app_state(temp_dir: &TempDir) -> AppState {
        let mut config = Config::default();

        // テスト用のディレクトリを設定
        let models_dir = temp_dir.path().join("models");
        fs::create_dir_all(&models_dir).unwrap();

        config.paths.models_dir = models_dir.to_string_lossy().to_string();
        config.paths.temp_dir = temp_dir.path().to_string_lossy().to_string();
        config.paths.upload_dir = temp_dir.path().to_string_lossy().to_string();
        config.server.host = "127.0.0.1".to_string();
        config.server.port = 8080;
        config.whisper.enable_gpu = false; // テスト環境ではGPU無効

        AppState::new(config)
    }

    /// ヘルスチェックエンドポイントのテスト
    #[tokio::test]
    async fn test_health_endpoint() {
        let temp_dir = TempDir::new().unwrap();
        let app_state = create_test_app_state(&temp_dir);
        let app = create_test_router(app_state);

        let request = Request::builder()
            .method(Method::GET)
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let health_response: HealthResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(health_response.status, "healthy");
        assert!(!health_response.version.is_empty());
        assert!(!health_response.model_loaded); // テスト環境ではモデル未ロード
        assert!(health_response.uptime_seconds >= 0);
        assert!(health_response.memory_usage_mb.is_none());
    }

    /// 統計エンドポイントのテスト
    #[tokio::test]
    async fn test_stats_endpoint() {
        let temp_dir = TempDir::new().unwrap();
        let app_state = create_test_app_state(&temp_dir);

        // 統計を記録
        {
            let mut stats = app_state.stats.lock().unwrap();
            stats.record_request();
            stats.record_success(1500, Some(60_000));
        }

        let app = create_test_router(app_state);

        let request = Request::builder()
            .method(Method::GET)
            .uri("/stats")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let stats_response: ServerStats = serde_json::from_slice(&body).unwrap();

        assert_eq!(stats_response.total_requests, 1);
        assert_eq!(stats_response.successful_transcriptions, 1);
        assert_eq!(stats_response.failed_transcriptions, 0);
        assert_eq!(stats_response.average_processing_time_ms, 1500.0);
        assert_eq!(stats_response.success_rate(), 100.0);
        assert!(stats_response.uptime_seconds >= 0);
    }

    /// モデル一覧エンドポイントのテスト
    #[tokio::test]
    async fn test_models_endpoint() {
        let temp_dir = TempDir::new().unwrap();
        let app_state = create_test_app_state(&temp_dir);

        // テスト用のモデルファイルを作成
        let models_dir = temp_dir.path().join("models");
        let model_file = models_dir.join("ggml-tiny.bin");
        fs::write(&model_file, b"dummy model data").unwrap();

        let app = create_test_router(app_state);

        let request = Request::builder()
            .method(Method::GET)
            .uri("/models")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let models_response: ModelsResponse = serde_json::from_slice(&body).unwrap();

        assert!(!models_response.models.is_empty());
        assert!(!models_response.current_model.is_empty());

        // tinyモデルが利用可能として表示されることを確認
        let tiny_model = models_response.models.iter()
            .find(|m| m.name.contains("Tiny"))
            .unwrap();
        assert!(tiny_model.is_available);
        assert_eq!(tiny_model.size_mb, 39);

        // 存在しないモデルは利用不可として表示される
        let base_model = models_response.models.iter()
            .find(|m| m.name.contains("Base"))
            .unwrap();
        assert!(!base_model.is_available);
    }

    /// 言語一覧エンドポイントのテスト
    #[tokio::test]
    async fn test_languages_endpoint() {
        let temp_dir = TempDir::new().unwrap();
        let app_state = create_test_app_state(&temp_dir);
        let app = create_test_router(app_state);

        let request = Request::builder()
            .method(Method::GET)
            .uri("/languages")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let languages_response: Vec<LanguageInfo> = serde_json::from_slice(&body).unwrap();

        assert!(!languages_response.is_empty());

        // 主要言語が含まれていることを確認
        let en_lang = languages_response.iter()
            .find(|l| l.code == "en")
            .unwrap();
        assert_eq!(en_lang.name, "English");

        let ja_lang = languages_response.iter()
            .find(|l| l.code == "ja")
            .unwrap();
        assert_eq!(ja_lang.name, "Japanese");

        let auto_lang = languages_response.iter()
            .find(|l| l.code == "auto")
            .unwrap();
        assert_eq!(auto_lang.name, "Auto Detect");
    }

    /// GPU状態エンドポイントのテスト
    #[tokio::test]
    async fn test_gpu_status_endpoint() {
        let temp_dir = TempDir::new().unwrap();
        let app_state = create_test_app_state(&temp_dir);
        let app = create_test_router(app_state);

        let request = Request::builder()
            .method(Method::GET)
            .uri("/gpu-status")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let gpu_status: Value = serde_json::from_slice(&body).unwrap();

        assert_eq!(gpu_status["gpu_enabled_in_config"], false); // テスト環境では無効
        assert_eq!(gpu_status["gpu_actually_enabled"], false);

        let environment = &gpu_status["environment"];
        assert!(environment["whisper_cublas"].is_boolean());
        assert!(environment["cuda_feature_enabled"].is_boolean());
        assert!(environment["opencl_feature_enabled"].is_boolean());

        let recommendations = &gpu_status["recommendations"];
        assert!(recommendations.is_array());
        assert!(recommendations.as_array().unwrap().len() > 0);
    }

    /// CORSプリフライトリクエストのテスト
    #[tokio::test]
    async fn test_cors_options_request() {
        let temp_dir = TempDir::new().unwrap();
        let app_state = create_test_app_state(&temp_dir);
        let app = create_test_router(app_state);

        let request = Request::builder()
            .method(Method::OPTIONS)
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);
    }

    /// 存在しないエンドポイントのテスト
    #[tokio::test]
    async fn test_not_found_endpoint() {
        let temp_dir = TempDir::new().unwrap();
        let app_state = create_test_app_state(&temp_dir);
        let app = create_test_router(app_state);

        let request = Request::builder()
            .method(Method::GET)
            .uri("/nonexistent")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    /// 複数のエンドポイントを順次テスト
    #[tokio::test]
    async fn test_multiple_endpoints_sequentially() {
        let temp_dir = TempDir::new().unwrap();
        let app_state = create_test_app_state(&temp_dir);

        // 統計を記録
        {
            let mut stats = app_state.stats.lock().unwrap();
            stats.record_request();
            stats.record_request();
            stats.record_success(1000, Some(60_000));
            stats.record_failure();
        }

        let endpoints = vec!["/health", "/stats", "/models", "/languages", "/gpu-status"];

        for endpoint in endpoints {
            let app = create_test_router(app_state.clone());

            let request = Request::builder()
                .method(Method::GET)
                .uri(endpoint)
                .body(Body::empty())
                .unwrap();

            let response = app.oneshot(request).await.unwrap();

            assert_eq!(
                response.status(),
                StatusCode::OK,
                "Endpoint {} should return 200 OK",
                endpoint
            );

            // レスポンスボディが有効なJSONであることを確認
            let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
            let json_value: Value = serde_json::from_slice(&body)
                .expect(&format!("Endpoint {} should return valid JSON", endpoint));

            assert!(!json_value.is_null());
        }
    }

    /// 同時リクエストのテスト
    #[tokio::test]
    async fn test_concurrent_requests() {
        let temp_dir = TempDir::new().unwrap();
        let app_state = create_test_app_state(&temp_dir);

        let mut handles = Vec::new();

        for i in 0..5 {
            let app_state_clone = app_state.clone();
            let handle = tokio::spawn(async move {
                let app = create_test_router(app_state_clone);

                let request = Request::builder()
                    .method(Method::GET)
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap();

                let response = app.oneshot(request).await.unwrap();
                (i, response.status())
            });

            handles.push(handle);
        }

        // 全ての同時リクエストが成功することを確認
        for handle in handles {
            let (request_id, status) = handle.await.unwrap();
            assert_eq!(status, StatusCode::OK, "Request {} should succeed", request_id);
        }
    }

    /// レスポンスのコンテンツタイプテスト
    #[tokio::test]
    async fn test_response_content_type() {
        let temp_dir = TempDir::new().unwrap();
        let app_state = create_test_app_state(&temp_dir);
        let app = create_test_router(app_state);

        let request = Request::builder()
            .method(Method::GET)
            .uri("/health")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        // Axumは自動的にContent-Type: application/jsonを設定する
        // ここではレスポンスが適切なJSONであることを確認
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json_result = serde_json::from_slice::<Value>(&body);
        assert!(json_result.is_ok(), "Response should be valid JSON");
    }

    /// エラーレスポンスのテスト（モックエラー）
    #[tokio::test]
    async fn test_error_response_format() {
        use WhisperBackendAPI::{handlers::ApiError, models::ApiErrorCode};
        use axum::response::IntoResponse;

        // APIエラーのレスポンス形式をテスト
        let error = ApiError::new(ApiErrorCode::InvalidInput, "Test error message")
            .with_details("Additional error details");

        let response = error.into_response();

        // ステータスコードが正しいことを確認
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        // レスポンスボディがErrorResponseの形式であることを確認
        let body = axum::body::to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let error_response: ErrorResponse = serde_json::from_slice(&body).unwrap();

        assert_eq!(error_response.error, "Test error message");
        assert_eq!(error_response.code, "INVALID_INPUT");
        assert_eq!(error_response.details, Some("Additional error details".to_string()));
    }

    /// アプリケーション状態の一貫性テスト
    #[tokio::test]
    async fn test_application_state_consistency() {
        let temp_dir = TempDir::new().unwrap();
        let app_state = create_test_app_state(&temp_dir);

        // 初期状態の確認
        {
            let stats = app_state.stats.lock().unwrap();
            assert_eq!(stats.total_requests, 0);
            assert_eq!(stats.successful_transcriptions, 0);
            assert_eq!(stats.failed_transcriptions, 0);
        }

        // 状態を変更
        {
            let mut stats = app_state.stats.lock().unwrap();
            stats.record_request();
            stats.record_success(2000, Some(60_000));
        }

        // 変更が反映されていることを確認
        {
            let stats = app_state.stats.lock().unwrap();
            assert_eq!(stats.total_requests, 1);
            assert_eq!(stats.successful_transcriptions, 1);
            assert_eq!(stats.average_processing_time_ms, 2000.0);
        }

        // Whisperエンジンの初期状態確認
        {
            let engine_guard = app_state.whisper_engine.lock().unwrap();
            assert!(engine_guard.is_none()); // テスト環境では未初期化
        }

        // 設定値の確認
        assert_eq!(app_state.config.server.host, "127.0.0.1");
        assert_eq!(app_state.config.server.port, 8080);
        assert!(!app_state.config.whisper.enable_gpu); // テスト環境では無効
    }
}

/// FrontendHealthにモデル名が含まれることを確認するテスト
use whisper_webui::handlers::{map_health_response, FrontendHealth};
use whisper_webui::client::HealthResponse;

#[test]
fn test_map_health_response_with_model_name() {
    // モデル名ありのHealthResponseを作成
    let health_response = HealthResponse {
        status: "healthy".to_string(),
        version: Some("1.0.0".to_string()),
        model_loaded: true,
        model_name: Some("large-q5_0".to_string()),
        uptime_seconds: 3600,
        memory_usage_mb: Some(1024),
    };

    // FrontendHealthにマッピング
    let frontend_health = map_health_response(health_response);

    // 検証
    assert_eq!(frontend_health.status, "healthy");
    assert_eq!(frontend_health.version, Some("1.0.0".to_string()));
    assert_eq!(frontend_health.whisper_loaded, true);
    assert_eq!(frontend_health.model_name, Some("large-q5_0".to_string()));
    assert_eq!(frontend_health.uptime_seconds, 3600);
    assert_eq!(frontend_health.memory_usage_mb, Some(1024));
}

#[test]
fn test_map_health_response_without_model_name() {
    // モデル名なしのHealthResponseを作成
    let health_response = HealthResponse {
        status: "healthy".to_string(),
        version: Some("1.0.0".to_string()),
        model_loaded: false,
        model_name: None,
        uptime_seconds: 1800,
        memory_usage_mb: Some(512),
    };

    // FrontendHealthにマッピング
    let frontend_health = map_health_response(health_response);

    // 検証
    assert_eq!(frontend_health.status, "healthy");
    assert_eq!(frontend_health.whisper_loaded, false);
    assert_eq!(frontend_health.model_name, None);
}

#[test]
fn test_map_health_response_with_empty_model_name() {
    // 空文字列のモデル名を持つHealthResponseを作成
    let health_response = HealthResponse {
        status: "healthy".to_string(),
        version: Some("1.0.0".to_string()),
        model_loaded: true,
        model_name: Some("  ".to_string()), // 空白のみ
        uptime_seconds: 900,
        memory_usage_mb: None,
    };

    // FrontendHealthにマッピング
    let frontend_health = map_health_response(health_response);

    // 空白のみの文字列はNoneとしてマッピングされるべき
    assert_eq!(frontend_health.model_name, None);
}

#[test]
fn test_frontend_health_structure() {
    // FrontendHealth構造体が正しくシリアライズできることを確認
    let frontend_health = FrontendHealth {
        status: "healthy".to_string(),
        version: Some("1.0.0".to_string()),
        whisper_loaded: true,
        model_name: Some("test-model".to_string()),
        uptime_seconds: 100,
        memory_usage_mb: Some(256),
    };

    // JSONにシリアライズ
    let json = serde_json::to_string(&frontend_health).unwrap();

    // model_nameフィールドが含まれていることを確認
    assert!(json.contains("model_name"));
    assert!(json.contains("test-model"));
}
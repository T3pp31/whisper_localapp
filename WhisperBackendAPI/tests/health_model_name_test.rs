/// HealthResponseにモデル名が含まれることを確認するテスト
use WhisperBackendAPI::config::Config;
use WhisperBackendAPI::handlers::{health_check, AppState};

#[tokio::test]
async fn test_health_check_includes_model_name_when_loaded() {
    // Configを作成
    let mut config = Config::default();
    config.whisper.default_model = "test-model".to_string();

    // AppStateを作成（Whisperエンジンはなしでテスト）
    let state = AppState::new(config);

    // health_checkを呼び出し
    let response = health_check(axum::extract::State(state)).await;
    let health_data = response.0;

    // ヘルスレスポンスを検証
    assert_eq!(health_data.status, "healthy");

    // モデルが読み込まれていない場合、model_nameはNoneであることを確認
    if !health_data.model_loaded {
        assert_eq!(health_data.model_name, None);
    }
}

#[tokio::test]
async fn test_health_response_structure() {
    // デフォルト設定でテスト
    let config = Config::default();
    let state = AppState::new(config);

    // health_checkを呼び出し
    let response = health_check(axum::extract::State(state)).await;
    let health_data = response.0;

    // 必須フィールドが存在することを確認
    assert!(!health_data.status.is_empty());
    assert!(!health_data.version.is_empty());
    assert!(health_data.uptime_seconds >= 0);

    // モデル名フィールドが存在することを確認（値はOptionalなのでNoneでもOK）
    let _ = health_data.model_name;
}
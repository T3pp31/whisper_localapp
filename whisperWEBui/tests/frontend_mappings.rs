use whisper_webui::client::{GpuModelInfo, GpuStatusResponse, HealthResponse, StatsResponse};
use whisper_webui::handlers::{map_gpu_status_response, map_health_response, map_stats_response};

#[test]
fn map_health_response_converts_fields() {
    let health = HealthResponse {
        status: "healthy".to_string(),
        version: Some(" 1.0.0 ".to_string()),
        model_loaded: true,
        uptime_seconds: 7_200,
        memory_usage_mb: Some(512),
    };

    let mapped = map_health_response(health);

    assert_eq!(mapped.status, "healthy");
    assert_eq!(mapped.version.as_deref(), Some("1.0.0"));
    assert!(mapped.whisper_loaded);
    assert_eq!(mapped.uptime_seconds, 7_200);
    assert_eq!(mapped.memory_usage_mb, Some(512));
}

#[test]
fn map_stats_response_converts_average_time() {
    let stats = StatsResponse {
        total_requests: 10,
        successful_transcriptions: 4,
        failed_transcriptions: 6,
        total_processing_time_ms: 15_000,
        average_processing_time_ms: 1_500.0,
        active_requests: 2,
        uptime_seconds: 3600,
    };

    let mapped = map_stats_response(stats);

    assert_eq!(mapped.requests_total, 10);
    assert_eq!(mapped.requests_successful, 4);
    assert_eq!(mapped.requests_failed, 6);
    assert_eq!(mapped.uptime_seconds, 3600);
    assert_eq!(mapped.average_processing_time, Some(1.5));
    assert_eq!(mapped.active_requests, 2);
}

#[test]
fn map_gpu_status_response_detects_gpu_availability() {
    let status = GpuStatusResponse {
        gpu_enabled_in_config: true,
        gpu_actually_enabled: false,
        model_info: Some(GpuModelInfo {
            is_loaded: true,
            language: Some("ja".to_string()),
            threads: 4,
            enable_gpu: true,
        }),
    };

    let mapped = map_gpu_status_response(status);

    assert!(mapped.gpu_available);
    assert_eq!(mapped.gpu_name.as_deref(), Some("GPU"));
    assert!(mapped.gpu_enabled_in_config);
}

#[test]
fn map_gpu_status_response_handles_disabled_gpu() {
    let status = GpuStatusResponse {
        gpu_enabled_in_config: false,
        gpu_actually_enabled: false,
        model_info: None,
    };

    let mapped = map_gpu_status_response(status);

    assert!(!mapped.gpu_available);
    assert_eq!(mapped.gpu_name, None);
    assert!(!mapped.gpu_enabled_in_config);
}

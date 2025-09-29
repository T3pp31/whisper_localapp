use whisper_webui::client::TimestampedTranscriptionResponse;

#[test]
fn parse_segments_only_payload() {
    let payload = r#"[
        {"text": "Hello", "start_time_ms": 0.0, "end_time_ms": 1500.0},
        {"text": " world", "start_time_ms": 1500.0, "end_time_ms": 2500.0}
    ]"#;

    let response = TimestampedTranscriptionResponse::from_backend_json(payload)
        .expect("segments only payload should parse correctly");

    assert_eq!(response.segments.len(), 2);
    assert_eq!(response.text, "Hello world");

    let duration = response
        .duration
        .expect("duration should be derived from segments");
    assert!((duration - 2.5).abs() < 1e-6);

    assert!(response.processing_time.is_none());
    assert!(response.language.is_none());
}

#[test]
fn parse_full_payload() {
    let payload = r#"{
        "text": "Hello world",
        "segments": [
            {"text": "Hello", "start": 0.0, "end": 1.0},
            {"text": " world", "start": 1.0, "end": 2.0}
        ],
        "language": "en",
        "duration": 2.5,
        "processing_time": 0.75
    }"#;

    let response = TimestampedTranscriptionResponse::from_backend_json(payload)
        .expect("full payload should parse correctly");

    assert_eq!(response.segments.len(), 2);
    assert_eq!(response.language.as_deref(), Some("en"));
    assert_eq!(response.duration, Some(2.5));
    assert_eq!(response.processing_time, Some(0.75));
    assert!((response.segments[1].end - 2.0).abs() < 1e-9);
}

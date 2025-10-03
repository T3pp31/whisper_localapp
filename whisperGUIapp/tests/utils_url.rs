use whisperGUIapp::utils::{build_full_url, is_allowed_audio_ext, REMOTE_ALLOWED_EXTS};

#[test]
fn build_full_url_handles_relative_endpoint_with_or_without_slash() {
    assert_eq!(
        build_full_url("http://localhost:8080", "/api"),
        "http://localhost:8080/api"
    );
    assert_eq!(
        build_full_url("http://localhost:8080/", "api"),
        "http://localhost:8080/api"
    );
}

#[test]
fn build_full_url_keeps_absolute_endpoint() {
    assert_eq!(
        build_full_url("http://localhost:8080", "https://example.com/x"),
        "https://example.com/x"
    );
}

#[test]
fn is_allowed_audio_ext_checks_extension_case_insensitively() {
    for ext in REMOTE_ALLOWED_EXTS {
        let path = format!("/tmp/test.{}", ext);
        assert!(is_allowed_audio_ext(&path));
        let path_upper = format!("/tmp/test.{}", ext.to_uppercase());
        assert!(is_allowed_audio_ext(&path_upper));
    }
    assert!(!is_allowed_audio_ext("/tmp/test.unknown"));
}


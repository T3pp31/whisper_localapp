use whisperGUIapp::config::{Config, WhisperConfig};

#[test]
fn default_timeout_is_600_seconds() {
    let cfg = Config::default();
    assert_eq!(cfg.whisper.request_timeout_secs, 600);
    assert_eq!(cfg.whisper.request_timeout_duration().as_secs(), 600);
}

#[test]
fn toml_without_timeout_field_uses_default() {
    // 既存の古い設定を想定し、timeoutフィールド無し
    let toml_str = r#"
        [whisper]
        model_path = "models/ggml-base.bin"
        default_model = "base"
        language = "ja"
        use_remote_server = false
        remote_server_url = "http://127.0.0.1:8080"
        remote_server_endpoint = "/transcribe-with-timestamps"

        [audio]
        sample_rate = 16000
        channels = 1
        buffer_size = 4096

        [gui]
        window_width = 800.0
        window_height = 600.0
        window_title = "Whisper音声文字起こし"
        theme = "Light"

        [performance]
        audio_threads = 2
        whisper_threads = 4
        use_gpu = false

        [paths]
        models_dir = "models"
        output_dir = "output"
        temp_dir = "temp"

        [output]
        default_format = "txt"
        supported_formats = ["txt", "srt", "vtt"]
        auto_save = true
    "#;

    let cfg: Config = toml::from_str(toml_str).expect("toml parse");
    assert_eq!(cfg.whisper.request_timeout_secs, 600);
}

#[test]
fn toml_with_timeout_overrides_default() {
    let toml_str = r#"
        [whisper]
        model_path = "models/ggml-base.bin"
        default_model = "base"
        language = "ja"
        use_remote_server = false
        remote_server_url = "http://127.0.0.1:8080"
        remote_server_endpoint = "/transcribe-with-timestamps"
        request_timeout_secs = 123

        [audio]
        sample_rate = 16000
        channels = 1
        buffer_size = 4096

        [gui]
        window_width = 800.0
        window_height = 600.0
        window_title = "Whisper音声文字起こし"
        theme = "Light"

        [performance]
        audio_threads = 2
        whisper_threads = 4
        use_gpu = false

        [paths]
        models_dir = "models"
        output_dir = "output"
        temp_dir = "temp"

        [output]
        default_format = "txt"
        supported_formats = ["txt", "srt", "vtt"]
        auto_save = true
    "#;

    let cfg: Config = toml::from_str(toml_str).expect("toml parse");
    assert_eq!(cfg.whisper.request_timeout_secs, 123);
    assert_eq!(cfg.whisper.request_timeout_duration().as_secs(), 123);
}


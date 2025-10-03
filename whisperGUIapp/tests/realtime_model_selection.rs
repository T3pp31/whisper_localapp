use std::fs;
use std::path::PathBuf;

use whisperGUIapp::config::{Config, PathsConfig, WhisperConfig, PerformanceConfig, AudioConfig, GuiConfig, OutputConfig};
use whisperGUIapp::realtime::resolve_realtime_model_path;

fn new_temp_dir() -> PathBuf {
    let mut base = std::env::temp_dir();
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    base.push(format!("wgui_rt_{}_{}", std::process::id(), millis));
    fs::create_dir_all(&base).unwrap();
    base
}

fn make_base_config(models_dir: &str, model_path: &str) -> Config {
    Config {
        whisper: WhisperConfig {
            model_path: model_path.to_string(),
            default_model: "base".to_string(),
            language: "ja".to_string(),
            use_remote_server: false,
            remote_server_url: "http://localhost:8080".to_string(),
            remote_server_endpoint: "/transcribe-with-timestamps".to_string(),
            request_timeout_secs: 10,
        },
        audio: AudioConfig { sample_rate: 16000, channels: 1, buffer_size: 4096 },
        gui: GuiConfig { window_width: 800.0, window_height: 600.0, window_title: "t".to_string(), theme: "Light".to_string() },
        performance: PerformanceConfig { audio_threads: 2, whisper_threads: 2, use_gpu: false },
        paths: PathsConfig { models_dir: models_dir.to_string(), output_dir: "output".to_string(), temp_dir: "temp".to_string() },
        output: OutputConfig { default_format: "txt".to_string(), supported_formats: vec!["txt".into()], auto_save: false },
    }
}

#[test]
fn resolve_uses_direct_model_path_when_provided() {
    let tmp = new_temp_dir();
    let models_dir = tmp.join("models");
    fs::create_dir_all(&models_dir).unwrap();

    // create dummy model file
    let custom_path = tmp.join("custom.bin");
    fs::write(&custom_path, b"dummy").unwrap();

    let cfg = make_base_config(models_dir.to_str().unwrap(), "unused.bin");
    let resolved = resolve_realtime_model_path(&cfg, None, Some(custom_path.to_str().unwrap())).unwrap();
    assert_eq!(PathBuf::from(resolved), custom_path);
}

#[test]
fn resolve_uses_model_id_when_downloaded() {
    let tmp = new_temp_dir();
    let models_dir = tmp.join("models");
    fs::create_dir_all(&models_dir).unwrap();

    // base model filename is ggml-base.bin in catalog
    let base_filename = "ggml-base.bin";
    let base_path = models_dir.join(base_filename);
    fs::write(&base_path, b"dummy").unwrap();

    let fallback = tmp.join("fallback.bin");
    fs::write(&fallback, b"fallback").unwrap();

    let cfg = make_base_config(models_dir.to_str().unwrap(), fallback.to_str().unwrap());
    let resolved = resolve_realtime_model_path(&cfg, Some("base"), None).unwrap();
    assert_eq!(PathBuf::from(resolved), base_path);
}

#[test]
fn resolve_falls_back_to_config_path() {
    let tmp = new_temp_dir();
    let models_dir = tmp.join("models");
    fs::create_dir_all(&models_dir).unwrap();

    let fallback = tmp.join("ggml-something.bin");
    fs::write(&fallback, b"fallback").unwrap();

    let cfg = make_base_config(models_dir.to_str().unwrap(), fallback.to_str().unwrap());
    let resolved = resolve_realtime_model_path(&cfg, None, None).unwrap();
    assert_eq!(PathBuf::from(resolved), fallback);
}

#[test]
fn resolve_errors_on_unknown_id_or_missing_file() {
    let tmp = new_temp_dir();
    let models_dir = tmp.join("models");
    fs::create_dir_all(&models_dir).unwrap();
    let cfg = make_base_config(models_dir.to_str().unwrap(), "unused.bin");

    // unknown id
    let err1 = resolve_realtime_model_path(&cfg, Some("unknown-id"), None).unwrap_err();
    assert!(format!("{}", err1).contains("未知のモデルID"));

    // known id but file missing: pick "tiny" known id but do not create file
    let err2 = resolve_realtime_model_path(&cfg, Some("tiny"), None).unwrap_err();
    assert!(format!("{}", err2).contains("未ダウンロード"));

    // direct path missing
    let err3 = resolve_realtime_model_path(&cfg, None, Some("/path/notfound.bin")).unwrap_err();
    assert!(format!("{}", err3).contains("見つかりません"));
}

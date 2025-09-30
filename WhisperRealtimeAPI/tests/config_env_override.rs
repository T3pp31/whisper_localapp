use std::fs;
use std::path::PathBuf;

use whisper_realtime_api::config::ConfigSet;

fn copy_default_config_to(dest: &PathBuf) {
    fs::create_dir_all(dest).expect("create temp config dir");
    let src = PathBuf::from("config");
    for name in [
        "system_requirements.yaml",
        "audio_processing.yaml",
        "asr_pipeline.yaml",
        "monitoring.yaml",
        "server.yaml",
        "whisper_model.yaml",
    ] {
        let from = src.join(name);
        let to = dest.join(name);
        fs::copy(&from, &to).unwrap_or_else(|e| panic!("copy {:?} -> {:?}: {}", from, to, e));
    }
}

#[test]
fn loads_config_from_env_dir() {
    let tmp = std::env::temp_dir().join(format!(
        "wra_cfg_{}",
        uuid::Uuid::new_v4()
    ));
    copy_default_config_to(&tmp);
    std::env::set_var(whisper_realtime_api::config::CONFIG_DIR_ENV, &tmp);

    let cfg = ConfigSet::load_from_env().expect("load config from env");
    assert_eq!(cfg.root(), tmp.as_path());
    assert!(!cfg.server.ws_bind_addr.is_empty());
    assert!(!cfg.server.asr_grpc_bind_addr.is_empty());
    assert!(!cfg.asr.service.endpoint.is_empty());
}

#[test]
fn audio_target_frame_samples_matches_yaml() {
    // 期待値: target.sample_rate_hz(16000) * frame_duration_ms(20) / 1000 = 320
    let cfg = ConfigSet::load_from_env().expect("load default config");
    assert_eq!(cfg.audio.target_frame_samples(), 320);
}


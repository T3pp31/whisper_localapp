use std::process::Command;

use whisper_realtime_api::config::ConfigSet;

fn parse_keyvals(stdout: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for line in stdout.lines() {
        if let Some((k, v)) = line.split_once('=') {
            map.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    map
}

#[test]
fn run_sh_dry_run_uses_yaml_defaults() {
    // 期待: 環境変数未指定時は YAML の値を採用
    let cfg = ConfigSet::load_from_dir("config").expect("load cfg");
    let out = Command::new("bash")
        .arg("-lc")
        .arg("./run.sh --dry-run")
        .output()
        .expect("run run.sh");
    assert!(out.status.success(), "run.sh exited with error: status={}", out.status);
    let stdout = String::from_utf8_lossy(&out.stdout);
    let kv = parse_keyvals(&stdout);
    assert_eq!(kv.get("ASR_ADDR").map(String::as_str), Some(cfg.server.asr_grpc_bind_addr.as_str()));
    assert_eq!(kv.get("HTTP_ADDR").map(String::as_str), Some(cfg.server.http_bind_addr.as_str()));
}

#[test]
fn run_sh_dry_run_applies_env_overrides() {
    let out = Command::new("bash")
        .arg("-lc")
        .arg("./run.sh --dry-run")
        .env("ASR_GRPC_BIND_ADDR", "127.0.0.1:55051")
        .env("HTTP_BIND_ADDR", "127.0.0.1:18080")
        .output()
        .expect("run run.sh");
    assert!(out.status.success(), "run.sh exited with error");
    let stdout = String::from_utf8_lossy(&out.stdout);
    let kv = parse_keyvals(&stdout);
    assert_eq!(kv.get("ASR_ADDR").map(String::as_str), Some("127.0.0.1:55051"));
    assert_eq!(kv.get("HTTP_ADDR").map(String::as_str), Some("127.0.0.1:18080"));
}


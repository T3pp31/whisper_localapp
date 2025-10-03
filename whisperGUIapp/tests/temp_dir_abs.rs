use std::path::PathBuf;
use whisperGUIapp::config::{Config, PathsConfig};

#[test]
fn absolute_temp_dir_returns_absolute_path_for_relative_config() {
    let mut cfg = Config::default();
    cfg.paths = PathsConfig { models_dir: "models".into(), output_dir: "output".into(), temp_dir: "temp_rel".into() };
    let abs = cfg.absolute_temp_dir();
    assert!(abs.is_absolute(), "expected absolute path, got {:?}", abs);
    assert!(abs.ends_with("temp_rel"));
}

#[test]
fn absolute_temp_dir_keeps_absolute_path() {
    let mut cfg = Config::default();
    let base = std::env::temp_dir();
    let p = base.join("wgui_temp_abs");
    cfg.paths.temp_dir = p.to_string_lossy().to_string();
    let abs = cfg.absolute_temp_dir();
    assert_eq!(abs, p);
}


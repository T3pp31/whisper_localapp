use std::fs;
use toml::Value;

#[test]
fn nvcc_prepend_flags_should_be_configured() {
    let config_path = "build_config.toml";
    let content =
        fs::read_to_string(config_path).expect("build_config.toml の読み込みに失敗しました");

    let value: Value = toml::from_str(&content)
        .expect("build_config.toml が正しい TOML フォーマットではありません");

    let flag_value = value
        .get("build")
        .and_then(|section| section.get("cuda"))
        .and_then(|cuda| cuda.get("nvcc_prepend_flags"))
        .and_then(|val| val.as_str())
        .expect("nvcc_prepend_flags が設定されていません");

    assert_eq!(flag_value, "-U_GNU_SOURCE");
}

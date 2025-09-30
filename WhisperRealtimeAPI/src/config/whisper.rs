use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct WhisperModelConfig {
    /// whisper.cpp ggml/gguf モデルファイルパス
    pub model_path: String,
    /// 使用スレッド数
    pub threads: usize,
    /// 言語（"auto" で自動判定）
    pub language: String,
    /// 翻訳（true: 多言語→英語に翻訳）
    pub translate: bool,
}


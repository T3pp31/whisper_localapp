//! Whisper モデルのカタログ定義と補助関数。
//! UI での選択やダウンロードに利用する。

use serde::Serialize;

/// カタログ上の 1 モデルを表す定義情報。
#[derive(Clone, Copy)]
pub struct ModelDefinition {
    pub id: &'static str,
    pub label: &'static str,
    pub filename: &'static str,
    pub url: &'static str,
    pub size_mb: Option<f64>,
}

/// 利用可能モデルのカタログ。
pub const MODEL_CATALOG: &[ModelDefinition] = &[
    ModelDefinition {
        id: "tiny",
        label: "Tiny (~39MB)",
        filename: "ggml-tiny.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-tiny.bin",
        size_mb: Some(39.0),
    },
    ModelDefinition {
        id: "base",
        label: "Base (~74MB)",
        filename: "ggml-base.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-base.bin",
        size_mb: Some(74.0),
    },
    ModelDefinition {
        id: "small",
        label: "Small (~244MB)",
        filename: "ggml-small.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-small.bin",
        size_mb: Some(244.0),
    },
    ModelDefinition {
        id: "medium",
        label: "Medium (~769MB)",
        filename: "ggml-medium.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-medium.bin",
        size_mb: Some(769.0),
    },
    ModelDefinition {
        id: "large",
        label: "Large v3 Turbo (~1.62GB)",
        filename: "ggml-large-v3-turbo.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo.bin",
        size_mb: Some(1620.0),
    },
    ModelDefinition {
        id: "large-q5_0",
        label: "Large v3 Turbo q5_0 (~548MB)",
        filename: "ggml-large-v3-turbo-q5_0.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large-v3-turbo-q5_0.bin",
        size_mb: Some(548.0),
    },
];

/// モデル ID から定義情報を取得する。
pub fn get_model_definition(id: &str) -> Option<&'static ModelDefinition> {
    MODEL_CATALOG.iter().find(|model| model.id == id)
}

/// UI 側に返すモデル情報。
#[derive(Serialize, Clone)]
pub struct ModelInfo {
    pub id: String,
    pub label: String,
    pub filename: String,
    pub path: String,
    pub downloaded: bool,
    pub current: bool,
    pub size_mb: Option<f64>,
}

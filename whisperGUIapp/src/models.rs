use serde::Serialize;

#[derive(Clone, Copy)]
pub struct ModelDefinition {
    pub id: &'static str,
    pub label: &'static str,
    pub filename: &'static str,
    pub url: &'static str,
    pub size_mb: Option<f64>,
}

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
        label: "Large (~1.55GB)",
        filename: "ggml-large.bin",
        url: "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/ggml-large.bin",
        size_mb: Some(1550.0),
    },
];

pub fn get_model_definition(id: &str) -> Option<&'static ModelDefinition> {
    MODEL_CATALOG.iter().find(|model| model.id == id)
}

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

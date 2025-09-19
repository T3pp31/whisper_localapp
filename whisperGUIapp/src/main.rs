#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

mod audio;
mod config;
mod models;
mod whisper;

use audio::{AudioProcessor};
use config::Config;
use models::{ModelDefinition, MODEL_CATALOG};
use whisper::WhisperEngine;

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::{ClipboardManager, State};

#[derive(Clone)]
struct AppState {
    config: Arc<Mutex<Config>>,
    whisper_engine: Arc<Mutex<Option<WhisperEngine>>>,
}

impl AppState {
    fn new() -> anyhow::Result<Self> {
        let config = Config::load()?;
        config.ensure_directories()?;

        Ok(Self {
            config: Arc::new(Mutex::new(config)),
            whisper_engine: Arc::new(Mutex::new(None)),
        })
    }
}

#[derive(Serialize, Deserialize)]
struct AudioMetadataResponse {
    duration: f32,
    sample_rate: u32,
}

#[derive(Serialize, Deserialize)]
struct TranscriptionResult {
    text: String,
    segments: usize,
}

#[derive(Serialize, Deserialize)]
struct ModelInfo {
    id: String,
    label: String,
    filename: String,
    path: String,
    downloaded: bool,
    current: bool,
    size_mb: Option<f64>,
}

// Tauri コマンドハンドラー
#[tauri::command]
fn select_audio_file() -> Result<String, String> {
    use tauri::api::dialog::blocking::FileDialogBuilder;

    let file_path = FileDialogBuilder::new()
        // mp4, wav を含む一般的な音声/動画コンテナを許可
        .add_filter("Audio Files", &["mp3", "wav", "m4a", "flac", "ogg", "mp4"])
        .pick_file();

    match file_path {
        Some(path) => Ok(path.to_string_lossy().to_string()),
        None => Err("ファイルが選択されませんでした".to_string()),
    }
}

#[tauri::command]
async fn load_audio_metadata(path: String, state: State<'_, AppState>) -> Result<AudioMetadataResponse, String> {
    let config = state.config.lock().map_err(|_| "設定の読み込みに失敗しました")?;

    let processor = AudioProcessor::new(&*config)
        .map_err(|e| format!("オーディオ処理の初期化に失敗しました: {}", e))?;

    let metadata = processor.probe_metadata(&path)
        .map_err(|e| format!("音声メタデータの取得に失敗しました: {}", e))?;

    Ok(AudioMetadataResponse {
        duration: metadata.duration_seconds,
        sample_rate: metadata.sample_rate,
    })
}

#[tauri::command]
async fn update_language_setting(language: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut config = state.config.lock().map_err(|_| "設定の更新に失敗しました")?;
    config.whisper.language = language;
    config.save().map_err(|e| format!("設定の保存に失敗しました: {}", e))?;

    // Whisperエンジンをリセット
    let mut engine = state.whisper_engine.lock().map_err(|_| "エンジンのリセットに失敗しました")?;
    *engine = None;

    Ok(())
}

#[tauri::command]
async fn start_transcription(
    audio_path: String,
    language: String,
    translate_to_english: bool,
    state: State<'_, AppState>,
) -> Result<TranscriptionResult, String> {
    let config_snapshot = {
        let config = state.config.lock().map_err(|_| "設定の読み込みに失敗しました")?;
        config.clone()
    };

    // 音声ファイルの読み込み
    let mut processor = AudioProcessor::new(&config_snapshot)
        .map_err(|e| format!("オーディオ処理の初期化に失敗しました: {}", e))?;

    let audio_data = processor
        .load_audio_file(&audio_path)
        .map_err(|e| format!("音声読み込みエラー: {}", e))?;

    // Whisperエンジンの初期化
    {
        let engine_guard = state.whisper_engine.lock()
            .map_err(|_| "Whisperエンジンのロックに失敗しました")?;

        if engine_guard.is_none() {
            drop(engine_guard);

            let engine = WhisperEngine::new(&config_snapshot.whisper.model_path, &config_snapshot)
                .map_err(|e| format!("モデルのロードに失敗しました: {}", e))?;

            let mut guard = state.whisper_engine.lock()
                .map_err(|_| "Whisperエンジンの設定に失敗しました")?;
            *guard = Some(engine);
        }
    }

    // 文字起こし実行
    let segments = {
        let engine_guard = state.whisper_engine.lock()
            .map_err(|_| "Whisperエンジンのロックに失敗しました")?;

        match engine_guard.as_ref() {
            Some(engine) => {
                let lang_opt = match language.trim() {
                    "" | "auto" => None,
                    other => Some(other),
                };
                engine
                    .transcribe_with_timestamps(&audio_data, translate_to_english, lang_opt)
                    .map_err(|e| format!("文字起こしに失敗しました: {}", e))
            }
            None => Err("Whisperエンジンが初期化されていません".to_string()),
        }?
    };

    let text = if segments.is_empty() {
        "(音声を認識できませんでした)".to_string()
    } else {
        segments
            .iter()
            .map(|segment| {
                format!(
                    "[{} --> {}] {}",
                    format_timestamp_ms(segment.start_time_ms),
                    format_timestamp_ms(segment.end_time_ms),
                    segment.text
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };

    Ok(TranscriptionResult {
        text,
        segments: segments.len(),
    })
}

#[tauri::command]
fn copy_to_clipboard(text: String, app: tauri::AppHandle) -> Result<(), String> {
    let mut clipboard = app.clipboard_manager();
    clipboard
        .write_text(text)
        .map_err(|e| format!("クリップボードへのコピーに失敗しました: {}", e))
}

#[tauri::command]
fn get_available_models(state: State<'_, AppState>) -> Result<Vec<ModelInfo>, String> {
    let config = state.config.lock().map_err(|_| "設定の読み込みに失敗しました")?;
    let models_dir = &config.paths.models_dir;
    let current_path = &config.whisper.model_path;

    let mut models = Vec::new();
    for model_def in MODEL_CATALOG {
        let model_path_buf = PathBuf::from(models_dir).join(model_def.filename);
        let downloaded = model_path_buf.exists();
        let current = current_path.contains(model_def.filename);

        let path = if current {
            current_path.clone()
        } else {
            model_path_buf.to_string_lossy().to_string()
        };

        models.push(ModelInfo {
            id: model_def.id.to_string(),
            label: model_def.label.to_string(),
            filename: model_def.filename.to_string(),
            path,
            downloaded,
            current,
            size_mb: model_def.size_mb,
        });
    }

    Ok(models)
}

#[tauri::command]
fn select_model(model_id: String, state: State<'_, AppState>) -> Result<String, String> {
    let model_def = MODEL_CATALOG
        .iter()
        .find(|m| m.id == model_id)
        .ok_or_else(|| format!("未知のモデルID: {}", model_id))?;

    let mut config = state.config.lock().map_err(|_| "設定の更新に失敗しました")?;
    let model_path = PathBuf::from(&config.paths.models_dir).join(model_def.filename);

    if !model_path.exists() {
        return Err(format!("モデルファイルが見つかりません: {}", model_def.filename));
    }

    config.whisper.model_path = model_path.to_string_lossy().to_string();
    config.whisper.default_model = model_id.clone();
    config.save().map_err(|e| format!("設定の保存に失敗しました: {}", e))?;

    // Whisperエンジンをリセット
    let mut engine = state.whisper_engine.lock().map_err(|_| "エンジンのリセットに失敗しました")?;
    *engine = None;

    Ok(format!("モデルを {} に切り替えました", model_def.label))
}

#[tauri::command]
fn select_model_file() -> Result<String, String> {
    use tauri::api::dialog::blocking::FileDialogBuilder;

    let file_path = FileDialogBuilder::new()
        .add_filter("Whisper Models", &["bin"])
        .pick_file();

    match file_path {
        Some(path) => Ok(path.to_string_lossy().to_string()),
        None => Err("ファイルが選択されませんでした".to_string()),
    }
}

fn main() {
    let app_state = AppState::new().expect("アプリケーションの初期化に失敗しました");

    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            select_audio_file,
            load_audio_metadata,
            update_language_setting,
            start_transcription,
            copy_to_clipboard,
            get_available_models,
            select_model,
            select_model_file
        ])
        .run(tauri::generate_context!())
        .expect("Tauriアプリケーションの実行に失敗しました");
}

fn format_timestamp_ms(ms: u64) -> String {
    let total_seconds = ms / 1000;
    let milliseconds = ms % 1000;
    let seconds = total_seconds % 60;
    let minutes = (total_seconds / 60) % 60;
    let hours = total_seconds / 3600;
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        hours, minutes, seconds, milliseconds
    )
}

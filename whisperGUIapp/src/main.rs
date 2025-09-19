#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod config;
mod models;
mod whisper;

use audio::AudioProcessor;
use config::Config;
use models::{get_model_definition, ModelInfo, MODEL_CATALOG};
use whisper::WhisperEngine;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tauri::Manager;

#[derive(Clone)]
struct AppState {
    config: Arc<Mutex<Config>>,
    whisper_engine: Arc<Mutex<Option<WhisperEngine>>>,
}

impl AppState {
    fn initialize() -> anyhow::Result<Self> {
        let config = Config::load()?;
        config.ensure_directories()?;

        Ok(Self {
            config: Arc::new(Mutex::new(config)),
            whisper_engine: Arc::new(Mutex::new(None)),
        })
    }
}

#[derive(Clone, serde::Serialize)]
struct StatusPayload {
    message: String,
}

#[derive(Clone, serde::Serialize)]
struct CompletedPayload {
    text: String,
    source_path: String,
}

#[derive(Clone, serde::Serialize)]
struct ModelDownloadPayload {
    model_id: String,
    status: String,
    message: String,
    progress: Option<f64>,
}

#[derive(Clone, serde::Serialize)]
struct ModelSelectedPayload {
    model_id: String,
    model_path: String,
}

#[tauri::command]
async fn list_models(state: tauri::State<'_, AppState>) -> Result<Vec<ModelInfo>, String> {
    let app_state = state.inner();
    let config_handle = app_state.config.clone();

    let (config_snapshot, models_dir) = match config_handle.lock() {
        Ok(guard) => (guard.clone(), guard.paths.models_dir.clone()),
        Err(_) => {
            return Err("設定の読み込みに失敗しました".into());
        }
    };

    let current_path = config_snapshot.whisper.model_path.clone();

    let mut items = Vec::new();

    for model in MODEL_CATALOG.iter() {
        let path_buf = PathBuf::from(&models_dir).join(model.filename);
        let path_str = path_buf.to_string_lossy().to_string();
        let downloaded = Path::new(&path_str).exists();
        let current = is_current_model(&current_path, &path_str, model.filename);

        items.push(ModelInfo {
            id: model.id.to_string(),
            label: model.label.to_string(),
            filename: model.filename.to_string(),
            path: path_str,
            downloaded,
            current,
            size_mb: model.size_mb,
        });
    }

    Ok(items)
}

#[tauri::command]
async fn select_model(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    model_id: String,
) -> Result<(), String> {
    let model_definition = get_model_definition(&model_id)
        .ok_or_else(|| format!("未知のモデルIDです: {}", model_id))?;

    let app_state = state.inner();
    let config_handle = app_state.config.clone();
    let whisper_engine_handle = app_state.whisper_engine.clone();

    let (models_dir, current_path) = match config_handle.lock() {
        Ok(guard) => (
            guard.paths.models_dir.clone(),
            guard.whisper.model_path.clone(),
        ),
        Err(_) => {
            return Err("設定の読み込みに失敗しました".into());
        }
    };

    let target_path = PathBuf::from(&models_dir).join(model_definition.filename);
    let target_path_str = target_path.to_string_lossy().to_string();

    if is_current_model(&current_path, &target_path_str, model_definition.filename) {
        return Ok(());
    }

    let needs_download = !target_path.exists();

    if needs_download {
        let download_app = app_handle.clone();
        let download_id = model_id.clone();
        let download_label = model_definition.label.to_string();
        let download_url = model_definition.url.to_string();
        let download_path = target_path.clone();

        tauri::async_runtime::spawn_blocking(move || {
            download_model_file(
                &download_app,
                &download_id,
                &download_label,
                &download_url,
                &download_path,
            )
        })
        .await
        .map_err(|err| format!("モデルダウンロードの実行に失敗しました: {}", err))?
        .map_err(|err| err.to_string())?;
    }

    {
        let mut config_guard = config_handle
            .lock()
            .map_err(|_| "設定の更新に失敗しました".to_string())?;
        config_guard.whisper.default_model = model_id.clone();
        config_guard.whisper.model_path = target_path_str.clone();
        config_guard
            .save()
            .map_err(|err| format!("設定ファイルの保存に失敗しました: {}", err))?;
    }

    {
        let mut engine_guard = whisper_engine_handle
            .lock()
            .map_err(|_| "Whisperエンジンのリセットに失敗しました".to_string())?;
        *engine_guard = None;
    }

    let _ = app_handle.emit_all(
        "model-selected",
        ModelSelectedPayload {
            model_id,
            model_path: target_path_str,
        },
    );

    Ok(())
}

fn download_model_file(
    app: &tauri::AppHandle,
    model_id: &str,
    label: &str,
    url: &str,
    destination: &Path,
) -> anyhow::Result<()> {
    let _ = app.emit_all(
        "model-download",
        ModelDownloadPayload {
            model_id: model_id.to_string(),
            status: "started".into(),
            message: format!("{} をダウンロードしています", label),
            progress: Some(0.0),
        },
    );

    let download_result = (|| -> anyhow::Result<()> {
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let client = reqwest::blocking::Client::new();
        let mut response = client.get(url).send()?.error_for_status()?;
        let total_size = response.content_length();
        let mut file = std::fs::File::create(destination)?;
        let mut buffer = [0u8; 8192];
        let mut downloaded: u64 = 0;
        let mut last_reported = 0.0;
        let mut emitted_unknown_progress = false;

        use std::io::{Read, Write};

        loop {
            let read_bytes = response.read(&mut buffer)?;
            if read_bytes == 0 {
                break;
            }

            file.write_all(&buffer[..read_bytes])?;
            downloaded += read_bytes as u64;

            if let Some(total) = total_size {
                if total > 0 {
                    let progress = (downloaded as f64 / total as f64) * 100.0;
                    if progress - last_reported >= 1.0 || progress >= 100.0 {
                        let clamped = progress.min(100.0);
                        let _ = app.emit_all(
                            "model-download",
                            ModelDownloadPayload {
                                model_id: model_id.to_string(),
                                status: "progress".into(),
                                message: format!("{} をダウンロード中 ({:.0}%)", label, clamped),
                                progress: Some(clamped),
                            },
                        );
                        last_reported = progress;
                    }
                }
            } else if !emitted_unknown_progress {
                let _ = app.emit_all(
                    "model-download",
                    ModelDownloadPayload {
                        model_id: model_id.to_string(),
                        status: "progress".into(),
                        message: format!("{} をダウンロード中", label),
                        progress: None,
                    },
                );
                emitted_unknown_progress = true;
            }
        }

        file.flush()?;

        Ok(())
    })();

    match download_result {
        Ok(()) => {
            let _ = app.emit_all(
                "model-download",
                ModelDownloadPayload {
                    model_id: model_id.to_string(),
                    status: "completed".into(),
                    message: format!("{} のダウンロードが完了しました", label),
                    progress: Some(100.0),
                },
            );
            Ok(())
        }
        Err(err) => {
            let _ = std::fs::remove_file(destination);
            let _ = app.emit_all(
                "model-download",
                ModelDownloadPayload {
                    model_id: model_id.to_string(),
                    status: "error".into(),
                    message: format!("{} のダウンロードに失敗しました: {}", label, err),
                    progress: None,
                },
            );
            Err(err)
        }
    }
}

#[tauri::command]
async fn transcribe_audio(
    app_handle: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
    path: String,
) -> Result<(), String> {
    if path.is_empty() {
        return Err("音声ファイルが選択されていません".into());
    }

    let app_state = state.inner();
    let whisper_engine = app_state.whisper_engine.clone();
    let config_state = app_state.config.clone();
    let app = app_handle.clone();

    let config_snapshot = match config_state.lock() {
        Ok(guard) => guard.clone(),
        Err(_) => {
            return Err("設定の読み込みに失敗しました".into());
        }
    };

    tauri::async_runtime::spawn_blocking(move || {
        let _ = app.emit_all(
            "transcription-started",
            StatusPayload {
                message: "文字起こしを開始します".into(),
            },
        );

        let mut processor = match AudioProcessor::new(&config_snapshot) {
            Ok(processor) => processor,
            Err(err) => {
                let _ = app.emit_all(
                    "transcription-error",
                    StatusPayload {
                        message: format!("オーディオ処理の初期化に失敗しました: {}", err),
                    },
                );
                return;
            }
        };

        let audio_data = match processor.load_audio_file(&path) {
            Ok(data) => data,
            Err(err) => {
                let _ = app.emit_all(
                    "transcription-error",
                    StatusPayload {
                        message: format!("音声読み込みエラー: {}", err),
                    },
                );
                return;
            }
        };

        let mut engine_guard = match whisper_engine.lock() {
            Ok(guard) => guard,
            Err(_) => {
                let _ = app.emit_all(
                    "transcription-error",
                    StatusPayload {
                        message: "Whisperエンジンの準備に失敗しました".into(),
                    },
                );
                return;
            }
        };

        if engine_guard.is_none() {
            let _ = app.emit_all(
                "transcription-progress",
                StatusPayload {
                    message: "モデルをロードしています".into(),
                },
            );

            match WhisperEngine::new(&config_snapshot.whisper.model_path, &config_snapshot) {
                Ok(engine) => {
                    *engine_guard = Some(engine);
                    let _ = app.emit_all(
                        "transcription-progress",
                        StatusPayload {
                            message: format!(
                                "モデルをロードしました: {}",
                                config_snapshot.whisper.model_path
                            ),
                        },
                    );
                }
                Err(err) => {
                    let _ = app.emit_all(
                        "transcription-error",
                        StatusPayload {
                            message: format!("モデルのロードに失敗しました: {}", err),
                        },
                    );
                    return;
                }
            }
        }

        let result = match engine_guard.as_ref() {
            Some(engine) => match engine.transcribe(&audio_data) {
                Ok(text) => text,
                Err(err) => {
                    let _ = app.emit_all(
                        "transcription-error",
                        StatusPayload {
                            message: format!("文字起こしに失敗しました: {}", err),
                        },
                    );
                    return;
                }
            },
            None => {
                let _ = app.emit_all(
                    "transcription-error",
                    StatusPayload {
                        message: "Whisperエンジンが初期化されませんでした".into(),
                    },
                );
                return;
            }
        };

        let _ = app.emit_all(
            "transcription-completed",
            CompletedPayload {
                text: result,
                source_path: path,
            },
        );
    });

    Ok(())
}

fn main() {
    let app_state = match AppState::initialize() {
        Ok(state) => state,
        Err(err) => {
            eprintln!("アプリケーション初期化エラー: {}", err);
            std::process::exit(1);
        }
    };

    tauri::Builder::default()
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            transcribe_audio,
            list_models,
            select_model
        ])
        .run(tauri::generate_context!())
        .expect("failed to run Whisper GUI app");
}

fn is_current_model(current_path: &str, candidate_path: &str, filename: &str) -> bool {
    if current_path.is_empty() {
        return false;
    }

    let current = Path::new(current_path);
    let candidate = Path::new(candidate_path);

    if current == candidate {
        return true;
    }

    if let Some(curr) = current.file_name() {
        if curr == std::ffi::OsStr::new(filename) {
            return true;
        }
    }

    false
}

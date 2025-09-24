#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

//! Tauri ベースの Whisper ローカルアプリ本体。
//! - 音声選択/プレビュー生成/文字起こしをコマンドとして提供
//! - モデルの一覧・選択・ダウンロード管理
//! - 設定の読み書きとユーザー領域への資産展開

mod audio;
mod config;
mod models;
mod whisper;

use audio::{AudioProcessor};
use config::Config;
use models::MODEL_CATALOG;
use whisper::WhisperEngine;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{ClipboardManager, Manager, State};
use std::io::{Read, Write};
use reqwest::header::CONTENT_TYPE;
use serde_json::Value as JsonValue;

/// Tauri 側で共有するアプリの状態。
/// - `config`: 現在のアプリ設定
/// - `whisper_engine`: ロード済み Whisper コンテキスト（必要時に初期化）
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

/// 音声メタデータの返却ペイロード。
#[derive(Serialize, Deserialize)]
struct AudioMetadataResponse {
    duration: f32,
    sample_rate: u32,
}

/// 文字起こし結果の返却ペイロード。
#[derive(Serialize, Deserialize)]
struct TranscriptionResult {
    text: String,
    segments: usize,
}

/// リモート GPU サーバ設定のシリアライズ用。
#[derive(Serialize, Deserialize)]
struct RemoteServerSettings {
    use_remote_server: bool,
    remote_server_url: String,
    remote_server_endpoint: String,
}

/// パフォーマンス設定（スレッド数など）の返却用。
#[derive(Serialize, Deserialize)]
struct PerformanceInfo {
    whisper_threads: usize,
    max_threads: usize,
}

/// モデル一覧表示用の情報。
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
/// ファイルダイアログで音声/動画ファイルを選択する。
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

/// ファイルのメタデータ（長さ・サンプリングレート）を取得する。
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

/// 16kHz モノラルのプレビュー用 WAV を temp に生成し、絶対パスを返す。
#[tauri::command]
async fn prepare_preview_wav(path: String, state: State<'_, AppState>) -> Result<String, String> {
    let config = state
        .config
        .lock()
        .map_err(|_| "設定の読み込みに失敗しました")?
        .clone();

    let mut processor = AudioProcessor::new(&config)
        .map_err(|e| format!("オーディオ処理の初期化に失敗しました: {}", e))?;

    // temp_dir が相対パスの場合でも、絶対パスに解決してから使用する
    let mut temp_dir = PathBuf::from(&config.paths.temp_dir);
    if temp_dir.is_relative() {
        // 実行ディレクトリを基準に絶対パスへ（失敗時はカレントディレクトリ）
        let base = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|p| p.to_path_buf()))
            .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        temp_dir = base.join(temp_dir);
    }

    // temp ディレクトリを確実に作成
    std::fs::create_dir_all(&temp_dir)
        .map_err(|e| format!("一時ディレクトリの作成に失敗しました: {}", e))?;

    let preview_path = temp_dir.join("preview.wav");

    // 既存のプレビューを削除
    if preview_path.exists() {
        let _ = std::fs::remove_file(&preview_path);
    }

    processor
        .decode_to_wav_file(&path, &preview_path.to_string_lossy())
        .map_err(|e| format!("プレビューWAV生成に失敗しました: {}", e))?;

    // 返却するパスは文字列の絶対パス
    Ok(preview_path.to_string_lossy().to_string())
}

/// UI から指定された言語設定を保存し、Whisper エンジンをリセット。
#[tauri::command]
async fn update_language_setting(language: String, state: State<'_, AppState>) -> Result<(), String> {
    let mut config = state.config.lock().map_err(|_| "設定の更新に失敗しました")?;
    config.whisper.language = language;
    #[cfg(not(debug_assertions))]
    {
        config
            .save()
            .map_err(|e| format!("設定の保存に失敗しました: {}", e))?;
    }

    // Whisperエンジンをリセット
    let mut engine = state.whisper_engine.lock().map_err(|_| "エンジンのリセットに失敗しました")?;
    *engine = None;

    Ok(())
}

/// 現在のパフォーマンス設定（Whisper スレッド数）と利用可能な最大スレッド数を返す。
#[tauri::command]
fn get_performance_info(state: State<'_, AppState>) -> Result<PerformanceInfo, String> {
    let cfg = state
        .config
        .lock()
        .map_err(|_| "設定の読み込みに失敗しました")?;

    let max_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .max(1);

    let wt = cfg.performance.whisper_threads.clamp(1, max_threads);
    Ok(PerformanceInfo {
        whisper_threads: wt,
        max_threads,
    })
}

/// Whisper のスレッド数を更新し、エンジンをリセットする。
#[tauri::command]
async fn update_whisper_threads(threads: usize, state: State<'_, AppState>) -> Result<(), String> {
    let max_threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .max(1);
    let clamped = threads.clamp(1, max_threads);

    // 設定を更新
    {
        let mut cfg = state
            .config
            .lock()
            .map_err(|_| "設定の更新に失敗しました")?;
        cfg.performance.whisper_threads = clamped;

        #[cfg(not(debug_assertions))]
        {
            cfg.save()
                .map_err(|e| format!("設定の保存に失敗しました: {}", e))?;
        }
    }

    // Whisper エンジンをリセット（次回実行時に新設定で初期化）
    let mut engine = state
        .whisper_engine
        .lock()
        .map_err(|_| "エンジンのリセットに失敗しました")?;
    *engine = None;

    Ok(())
}

/// 指定音声の文字起こしを実行。言語・翻訳有無を受け取り、
/// タイムスタンプ付きセグメントを結合したテキストを返す。
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

    // リモート GPU サーバを利用する場合は HTTP 経由で実行
    if config_snapshot.whisper.use_remote_server {
        return transcribe_via_remote(
            &audio_path,
            &language,
            translate_to_english,
            &config_snapshot.whisper.remote_server_url,
            &config_snapshot.whisper.remote_server_endpoint,
            &config_snapshot,
        ).await;
    }

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

/// リモート GPU サーバへ音声ファイルを送信してタイムスタンプ付き文字起こしを取得。
async fn transcribe_via_remote(
    audio_path: &str,
    language: &str,
    translate_to_english: bool,
    base_url: &str,
    endpoint_path: &str,
    config: &Config,
) -> Result<TranscriptionResult, String> {
    // エンドポイントを組み立て（endpoint が絶対URLなら優先）
    let endpoint_trimmed = endpoint_path.trim();
    let endpoint_full = if endpoint_trimmed.starts_with("http://") || endpoint_trimmed.starts_with("https://") {
        endpoint_trimmed.to_string()
    } else {
        let base = base_url.trim().trim_end_matches('/');
        let ep_owned = if endpoint_trimmed.starts_with('/') {
            endpoint_trimmed.to_string()
        } else {
            format!("/{}", endpoint_trimmed)
        };
        format!("{}{}", base, ep_owned)
    };

    // サーバ許容拡張子チェック（wav / mp3 / m4a / flac / ogg）。
    // 非対応の場合は一時WAVへ変換して送信。
    let allowed_exts = ["wav", "mp3", "m4a", "flac", "ogg"];
    let orig_path = std::path::Path::new(audio_path);
    let ext_ok = orig_path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| allowed_exts.contains(&s.to_lowercase().as_str()))
        .unwrap_or(false);

    // 変換先のアップロードパスを決定
    let upload_path: std::path::PathBuf = if ext_ok {
        orig_path.to_path_buf()
    } else {
        // temp ディレクトリを解決
        let mut temp_dir = std::path::PathBuf::from(&config.paths.temp_dir);
        if temp_dir.is_relative() {
            let base = std::env::current_exe()
                .ok()
                .and_then(|p| p.parent().map(|p| p.to_path_buf()))
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from(".")));
            temp_dir = base.join(temp_dir);
        }
        std::fs::create_dir_all(&temp_dir)
            .map_err(|e| format!("一時ディレクトリの作成に失敗しました: {}", e))?;

        let target = temp_dir.join("remote_upload.wav");
        // 変換実施
        let mut processor = AudioProcessor::new(config)
            .map_err(|e| format!("オーディオ処理の初期化に失敗しました: {}", e))?;
        processor
            .decode_to_wav_file(audio_path, &target.to_string_lossy())
            .map_err(|e| format!("GPUサーバ用のWAV変換に失敗しました: {}", e))?;
        target
    };

    // ファイルを読み込んで固定長のバイト列として送信（curl に近い挙動）
    let bytes = std::fs::read(&upload_path)
        .map_err(|e| format!("音声ファイルの読み込みに失敗しました: {}", e))?;
    let filename = upload_path
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("audio");

    // multipart フォームを構築（MIME を簡易推定）
    // curl の既定に合わせて application/octet-stream を強制
    let mut file_part = reqwest::multipart::Part::bytes(bytes)
        .file_name(filename.to_string());
    let _ = {
        file_part = file_part.mime_str("application/octet-stream")
            .map_err(|e| format!("MIME設定に失敗しました: {}", e))?;
        &file_part
    };
    let mut form = reqwest::multipart::Form::new()
        .part("file", file_part)
        .text("translate_to_english", if translate_to_english { "true" } else { "false" }.to_string());
    let lang_trim = language.trim();
    if !lang_trim.is_empty() && lang_trim != "auto" {
        form = form.text("language", lang_trim.to_string());
    }

    // リクエスト送信
    let client = reqwest::Client::new();
    let resp = client
        .post(&endpoint_full)
        .multipart(form)
        .send()
        .await
        .map_err(|e| format!("GPUサーバへの接続に失敗しました: {}", e))?;

    if !resp.status().is_success() {
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        let mut snippet = body.trim().to_string();
        if snippet.len() > 500 { snippet.truncate(500); snippet.push_str(" …"); }
        return Err(format!("GPUサーバがエラーを返しました: HTTP {} {} -> {}", status, endpoint_full, snippet));
    }

    // 応答解析: JSONの場合は text/segments を解釈。非JSONはテキストとして採用。
    let ct = resp
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_lowercase();

    if ct.starts_with("application/json") {
        let json: JsonValue = resp.json().await.map_err(|e| format!("JSON の解析に失敗しました: {}", e))?;
        // text があれば優先
        if let Some(t) = json.get("text").and_then(|v| v.as_str()) {
            let segments = if let Some(arr) = json.get("segments").and_then(|v| v.as_array()) {
                arr.len()
            } else { 1 };
            return Ok(TranscriptionResult { text: t.to_string(), segments });
        }
        // segments からテキストを再構成
        if let Some(arr) = json.get("segments").and_then(|v| v.as_array()) {
            let mut lines: Vec<String> = Vec::new();
            for seg in arr {
                let text = seg.get("text").and_then(|v| v.as_str()).unwrap_or("");
                // 秒 or ミリ秒のどちらかで与えられている前提で吸収
                let (start_ms, end_ms) = if let (Some(s), Some(e)) = (seg.get("start_time_ms").and_then(|v| v.as_u64()), seg.get("end_time_ms").and_then(|v| v.as_u64())) {
                    (s, e)
                } else {
                    let s = seg.get("start").and_then(|v| v.as_f64()).unwrap_or(0.0);
                    let e = seg.get("end").and_then(|v| v.as_f64()).unwrap_or(s);
                    ((s * 1000.0) as u64, (e * 1000.0) as u64)
                };
                lines.push(format!(
                    "[{} --> {}] {}",
                    format_timestamp_ms(start_ms),
                    format_timestamp_ms(end_ms),
                    text
                ));
            }
            let text = lines.join("\n");
            return Ok(TranscriptionResult { text, segments: arr.len() });
        }
        // それ以外の JSON は文字列化
        let text = json.to_string();
        return Ok(TranscriptionResult { text, segments: 1 });
    } else {
        let body = resp.text().await.map_err(|e| format!("応答読み取りに失敗しました: {}", e))?;
        // セグメント数は大まかに行数で推定
        let segments = body.lines().filter(|l| !l.trim().is_empty()).count();
        let segments = if segments == 0 { 1 } else { segments };
        return Ok(TranscriptionResult { text: body, segments });
    }
}

fn guess_mime_from_filename(name: &str) -> Option<String> {
    let lower = name.to_lowercase();
    let m = if lower.ends_with(".wav") {
        "audio/wav"
    } else if lower.ends_with(".mp3") {
        "audio/mpeg"
    } else if lower.ends_with(".m4a") || lower.ends_with(".mp4") || lower.ends_with(".aac") {
        "audio/mp4"
    } else if lower.ends_with(".ogg") || lower.ends_with(".oga") {
        "audio/ogg"
    } else if lower.ends_with(".flac") {
        "audio/flac"
    } else {
        "application/octet-stream"
    };
    Some(m.to_string())
}

/// リモート GPU サーバ設定の取得。
#[tauri::command]
fn get_remote_server_settings(state: State<'_, AppState>) -> Result<RemoteServerSettings, String> {
    let cfg = state.config.lock().map_err(|_| "設定の読み込みに失敗しました")?;
    Ok(RemoteServerSettings {
        use_remote_server: cfg.whisper.use_remote_server,
        remote_server_url: cfg.whisper.remote_server_url.clone(),
        remote_server_endpoint: cfg.whisper.remote_server_endpoint.clone(),
    })
}

/// リモート GPU サーバ設定の更新（保存し、ローカルエンジンをリセット）。
#[tauri::command]
async fn update_remote_server_settings(use_remote_server: bool, remote_server_url: String, remote_server_endpoint: String, state: State<'_, AppState>) -> Result<(), String> {
    // URL は空白をトリム
    let url = remote_server_url.trim().to_string();
    let ep = remote_server_endpoint.trim().to_string();
    let mut cfg = state.config.lock().map_err(|_| "設定の更新に失敗しました")?;
    cfg.whisper.use_remote_server = use_remote_server;
    if !url.is_empty() { cfg.whisper.remote_server_url = url; }
    if !ep.is_empty() { cfg.whisper.remote_server_endpoint = ep; }
    #[cfg(not(debug_assertions))]
    {
        cfg.save().map_err(|e| format!("設定の保存に失敗しました: {}", e))?;
    }
    // ローカルエンジンは未使用/切替のため破棄
    let mut engine = state.whisper_engine.lock().map_err(|_| "エンジンのリセットに失敗しました")?;
    *engine = None;
    Ok(())
}

/// 結果テキストをクリップボードへコピー。
#[tauri::command]
fn copy_to_clipboard(text: String, app: tauri::AppHandle) -> Result<(), String> {
    let mut clipboard = app.clipboard_manager();
    clipboard
        .write_text(text)
        .map_err(|e| format!("クリップボードへのコピーに失敗しました: {}", e))
}

/// モデルダウンロードの進捗イベント。
#[derive(Serialize, Clone)]
struct DownloadProgressPayload {
    id: String,
    filename: String,
    downloaded: u64,
    total: Option<u64>,
    phase: String,     // start | progress | done | error
    message: Option<String>,
}

fn emit_progress(app: &tauri::AppHandle, payload: &DownloadProgressPayload) {
    let _ = app.emit_all("download-progress", payload.clone());
}

/// 指定のモデル 1 件をダウンロードして保存する。
#[tauri::command]
async fn download_model(model_id: String, app: tauri::AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    let (url, filename) = {
        let def = models::get_model_definition(&model_id)
            .ok_or_else(|| format!("未知のモデルID: {}", model_id))?;
        (def.url.to_string(), def.filename.to_string())
    };

    let models_dir = {
        let cfg = state
            .config
            .lock()
            .map_err(|_| "設定の読み込みに失敗しました")?;
        std::path::PathBuf::from(&cfg.paths.models_dir)
    };
    std::fs::create_dir_all(&models_dir)
        .map_err(|e| format!("models ディレクトリの作成に失敗しました: {}", e))?;

    let dest = models_dir.join(&filename);
    if dest.exists() {
        return Ok(format!("{} は既に存在します", filename));
    }

    // ブロッキングダウンロードを別スレッドで実行
    let dest_cloned = dest.clone();
    let url_cloned = url.clone();
    let app_cloned = app.clone();
    let model_id_cloned = model_id.clone();
    let filename_cloned = filename.clone();
    tokio::task::spawn_blocking(move || {
        download_to_file_with_progress(&app_cloned, &model_id_cloned, &filename_cloned, &url_cloned, &dest_cloned)
    })
    .await
    .map_err(|e| format!("ダウンロードスレッドの実行に失敗しました: {}", e))?
    .map_err(|e| format!("ダウンロードに失敗しました: {}", e))?;

    Ok(format!("{} をダウンロードしました", filename))
}

/// 既知の全モデルをダウンロードする（存在するものはスキップ）。
#[tauri::command]
async fn download_all_models(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let models_dir = {
        let cfg = state
            .config
            .lock()
            .map_err(|_| "設定の読み込みに失敗しました")?;
        std::path::PathBuf::from(&cfg.paths.models_dir)
    };
    std::fs::create_dir_all(&models_dir)
        .map_err(|e| format!("models ディレクトリの作成に失敗しました: {}", e))?;

    let mut downloaded = Vec::new();
    for def in MODEL_CATALOG {
        let dest = models_dir.join(def.filename);
        if dest.exists() {
            continue;
        }
        let url = def.url.to_string();
        let dest_cloned = dest.clone();
        let app_cloned = app.clone();
        let id = def.id.to_string();
        let filename = def.filename.to_string();
        tokio::task::spawn_blocking(move || download_to_file_with_progress(&app_cloned, &id, &filename, &url, &dest_cloned))
            .await
            .map_err(|e| format!("ダウンロードスレッドの実行に失敗しました: {}", e))?
            .map_err(|e| format!("{} のダウンロードに失敗しました: {}", def.filename, e))?;
        downloaded.push(def.filename.to_string());
    }
    Ok(downloaded)
}

/// ブロッキングダウンロード実体。定期的に進捗イベントを発火する。
fn download_to_file_with_progress(
    app: &tauri::AppHandle,
    model_id: &str,
    filename: &str,
    url: &str,
    dest: &std::path::Path,
) -> anyhow::Result<()> {
    let client = reqwest::blocking::Client::builder().build()?;
    let mut resp = client.get(url).send()?;
    if !resp.status().is_success() {
        let msg = format!("HTTP {}", resp.status());
        emit_progress(
            app,
            &DownloadProgressPayload {
                id: model_id.to_string(),
                filename: filename.to_string(),
                downloaded: 0,
                total: None,
                phase: "error".into(),
                message: Some(msg.clone()),
            },
        );
        return Err(anyhow::anyhow!(msg));
    }

    let total = resp.content_length();
    emit_progress(
        app,
        &DownloadProgressPayload {
            id: model_id.to_string(),
            filename: filename.to_string(),
            downloaded: 0,
            total,
            phase: "start".into(),
            message: None,
        },
    );

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = dest.with_extension("part");
    let mut file = std::fs::File::create(&tmp)?;
    let mut buf = [0u8; 1024 * 1024];
    let mut downloaded: u64 = 0;

    loop {
        let n = resp.read(&mut buf)? as u64;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..(n as usize)])?;
        downloaded = downloaded.saturating_add(n);
        emit_progress(
            app,
            &DownloadProgressPayload {
                id: model_id.to_string(),
                filename: filename.to_string(),
                downloaded,
                total,
                phase: "progress".into(),
                message: None,
            },
        );
    }
    drop(file);
    std::fs::rename(&tmp, dest)?;

    emit_progress(
        app,
        &DownloadProgressPayload {
            id: model_id.to_string(),
            filename: filename.to_string(),
            downloaded: total.unwrap_or(downloaded),
            total,
            phase: "done".into(),
            message: None,
        },
    );
    Ok(())
}
/// 利用可能なモデルの一覧を返す（ダウンロード状況や選択中のモデル含む）。
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

/// モデル ID を指定して現在の使用モデルを切り替える。
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
    // 開発時のホットリロード回避: debug ビルドではファイル保存をスキップ
    #[cfg(not(debug_assertions))]
    {
        config
            .save()
            .map_err(|e| format!("設定の保存に失敗しました: {}", e))?;
    }

    // Whisperエンジンをリセット
    let mut engine = state.whisper_engine.lock().map_err(|_| "エンジンのリセットに失敗しました")?;
    *engine = None;

    Ok(format!("モデルを {} に切り替えました", model_def.label))
}

/// ファイルダイアログからローカルのモデルファイル（.bin）を選択する。
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
        .setup(|app| {
            // ユーザー領域の models ディレクトリへリソースから同梱モデルを展開
            let state = app.state::<AppState>();
            let mut config_guard = state
                .config
                .lock()
                .map_err(|_| "設定のロックに失敗しました")?;

            // 既定の models_dir をユーザー領域へ移行（例: %LOCALAPPDATA%/whisperGUIapp/models）
            let user_models_dir = dirs_next::data_local_dir()
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
                .join("whisperGUIapp")
                .join("models");

            if config_guard.paths.models_dir == "models" {
                config_guard.paths.models_dir = user_models_dir.to_string_lossy().to_string();
            }
            // 確実に作成
            let _ = std::fs::create_dir_all(&config_guard.paths.models_dir);

            // リソースディレクトリ内の models を探す（複数パターンを試す）
            let mut resource_models_dir: Option<std::path::PathBuf> = None;
            if let Some(p) = app.path_resolver().resolve_resource("models") {
                if p.exists() { resource_models_dir = Some(p); }
            }
            if resource_models_dir.is_none() {
                if let Some(p) = app.path_resolver().resolve_resource("resources/models") {
                    if p.exists() { resource_models_dir = Some(p); }
                }
            }
            if resource_models_dir.is_none() {
                // フォールバック: 実行ファイルと同階層の resources/models
                if let Ok(exe) = std::env::current_exe() {
                    if let Some(dir) = exe.parent() {
                        let cand = dir.join("resources").join("models");
                        if cand.exists() { resource_models_dir = Some(cand); }
                    }
                }
            }

            if let Some(src_models) = resource_models_dir {
                if src_models.exists() {
                    if let Ok(entries) = std::fs::read_dir(&src_models) {
                        for e in entries.flatten() {
                            if let Ok(ft) = e.file_type() {
                                if ft.is_file() {
                                    let file_name = e.file_name();
                                    let dest = std::path::Path::new(&config_guard.paths.models_dir).join(file_name);
                                    if !dest.exists() {
                                        let _ = std::fs::copy(e.path(), &dest);
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // モデルパスが存在しない場合は、新しい models_dir にある既定モデルへ切替
            if !std::path::Path::new(&config_guard.whisper.model_path).exists() {
                let default_id = config_guard.whisper.default_model.clone();
                let mut candidate = None;
                if let Some(def) = crate::models::get_model_definition(&default_id) {
                    let p = std::path::Path::new(&config_guard.paths.models_dir).join(def.filename);
                    if p.exists() { candidate = Some(p); }
                }
                if candidate.is_none() {
                    // カタログ中で存在する最初のモデル
                    for def in crate::models::MODEL_CATALOG {
                        let p = std::path::Path::new(&config_guard.paths.models_dir).join(def.filename);
                        if p.exists() { candidate = Some(p); break; }
                    }
                }
                if let Some(p) = candidate {
                    config_guard.whisper.model_path = p.to_string_lossy().to_string();
                }
            }

            // 設定をユーザー領域へ保存（リリース時のみ）
            #[cfg(not(debug_assertions))]
            {
                let _ = config_guard.save();
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            select_audio_file,
            load_audio_metadata,
            prepare_preview_wav,
            update_language_setting,
            get_performance_info,
            update_whisper_threads,
            start_transcription,
            get_remote_server_settings,
            update_remote_server_settings,
            copy_to_clipboard,
            get_available_models,
            select_model,
            select_model_file,
            download_model,
            download_all_models
        ])
        .run(tauri::generate_context!())
        .expect("Tauriアプリケーションの実行に失敗しました");
}

/// 000:00:00.000 形式のタイムスタンプ文字列へ変換。
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

use anyhow::Result;
use once_cell::sync::OnceCell;
use rubato::{Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction};
use serde::Serialize;
use std::collections::VecDeque;
use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};
use std::time::{Duration, Instant};

use crate::config::Config;
use crate::whisper::WhisperEngine;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use tauri::Manager;

static MANAGER: OnceCell<Mutex<RealtimeManager>> = OnceCell::new();

pub fn manager() -> &'static Mutex<RealtimeManager> {
    MANAGER.get_or_init(|| Mutex::new(RealtimeManager::new()))
}

#[derive(Serialize, Clone, Default)]
pub struct RealtimeStatus {
    pub running: bool,
    pub phase: String,
    pub message: Option<String>,
}

pub struct RealtimeManager {
    running: Arc<AtomicBool>,
    recog_thread: Option<std::thread::JoinHandle<()>>,
    ring: Arc<Mutex<VecDeque<f32>>>,
    input_sr: Arc<Mutex<u32>>, // input sample rate
}

impl RealtimeManager {
    fn new() -> Self {
        Self {
            running: Arc::new(AtomicBool::new(false)),
            recog_thread: None,
            ring: Arc::new(Mutex::new(VecDeque::with_capacity(48000 * 30))),
            input_sr: Arc::new(Mutex::new(48000)),
        }
    }

    pub fn status(&self) -> RealtimeStatus {
        let running = self.running.load(Ordering::SeqCst);
        RealtimeStatus { running, phase: if running { "running".into() } else { "stopped".into() }, message: None }
    }

    pub fn start(&mut self, app: tauri::AppHandle, device_name: Option<String>, language: Option<String>, threads: Option<usize>) -> Result<()> {
        if self.running.load(Ordering::SeqCst) {
            return Ok(());
        }

        self.emit_status(&app, "starting", None);
        // Prepare ring buffer
        if let Ok(mut ring) = self.ring.lock() { ring.clear(); }
        self.running.store(true, Ordering::SeqCst);

        // Spawn capture + recognition thread
        let app_clone = app.clone();
        let ring2 = Arc::clone(&self.ring);
        let running2 = Arc::clone(&self.running);
        let input_sr2 = Arc::clone(&self.input_sr);
        let device_name_clone = device_name.clone();
        let language_override = language.clone();
        let threads_override = threads.clone();

        self.recog_thread = Some(std::thread::spawn(move || {
            // Load config and engine inside thread (avoid Send bound issues)
            let cfg = match Config::load() {
                Ok(c) => c,
                Err(e) => {
                    let _ = app_clone.emit_all("realtime-status", serde_json::json!({
                        "phase": "error",
                        "message": format!("設定読み込み失敗: {}", e)
                    }));
                    running2.store(false, Ordering::SeqCst);
                    return;
                }
            };
            let mut engine = match WhisperEngine::new(&cfg.whisper.model_path, &cfg) {
                Ok(e) => e,
                Err(e) => {
                    let _ = app_clone.emit_all("realtime-status", serde_json::json!({
                        "phase": "error",
                        "message": format!("モデル初期化失敗: {}", e)
                    }));
                    running2.store(false, Ordering::SeqCst);
                    return;
                }
            };

            // スレッド数のオーバーライドがあれば適用
            if let Some(t) = threads_override { engine.set_threads(t); }

            // Setup CPAL inside thread; keep stream owned here
            let host = cpal::default_host();
            let device = if let Some(name) = device_name_clone {
                host.input_devices()
                    .ok()
                    .and_then(|mut it| it.find(|d| d.name().ok().as_deref() == Some(name.as_str())))
                    .unwrap_or_else(|| host.default_input_device().expect("no input device available"))
            } else {
                host.default_input_device().expect("no input device available")
            };
            let supported_config = match device.default_input_config() {
                Ok(c) => c,
                Err(e) => {
                    let _ = app_clone.emit_all("realtime-status", serde_json::json!({
                        "phase": "error",
                        "message": format!("入力デバイス初期化失敗: {}", e)
                    }));
                    running2.store(false, Ordering::SeqCst);
                    return;
                }
            };
            let sample_rate = supported_config.sample_rate().0;
            if let Ok(mut sr) = input_sr2.lock() { *sr = sample_rate; }
            let channels = supported_config.channels() as usize;
            let stream_config: cpal::StreamConfig = supported_config.clone().into();
            let ring = Arc::clone(&ring2);
            let running = Arc::clone(&running2);

            let stream_res = match supported_config.sample_format() {
                cpal::SampleFormat::F32 => build_stream_f32(&device, &stream_config, channels, ring, running.clone()),
                cpal::SampleFormat::I16 => build_stream_i16(&device, &stream_config, channels, ring, running.clone()),
                cpal::SampleFormat::U16 => build_stream_u16(&device, &stream_config, channels, ring, running.clone()),
                other => {
                    let _ = app_clone.emit_all("realtime-status", serde_json::json!({
                        "phase": "error",
                        "message": format!("未対応のサンプル形式: {:?}", other)
                    }));
                    running2.store(false, Ordering::SeqCst);
                    return;
                }
            };
            let stream = match stream_res {
                Ok(s) => s,
                Err(e) => {
                    let _ = app_clone.emit_all("realtime-status", serde_json::json!({
                        "phase": "error",
                        "message": format!("入力ストリーム作成失敗: {}", e)
                    }));
                    running2.store(false, Ordering::SeqCst);
                    return;
                }
            };
            if let Err(e) = stream.play() {
                let _ = app_clone.emit_all("realtime-status", serde_json::json!({
                    "phase": "error",
                    "message": format!("録音開始失敗: {}", e)
                }));
                running2.store(false, Ordering::SeqCst);
                return;
            }

            let mut last_level_emit = Instant::now();
            let mut last_recog = Instant::now();
            let level_interval = Duration::from_millis(200);
            let recog_interval = Duration::from_millis(1000);
            let window_secs: f32 = 3.0;

            let mut prev_partial = String::new();
            let mut step = 0u64;

            // Ready
            let _ = app_clone.emit_all("realtime-status", serde_json::json!({ "phase": "running" }));

            loop {
                if !running2.load(Ordering::SeqCst) { break; }

                let input_sr: u32 = match input_sr2.lock() { Ok(g) => *g, Err(_) => 48000 };
                // Make a snapshot copy of last N samples
                let (buf_vec, _buf_len) = {
                    let mut v: Vec<f32> = Vec::new();
                    if let Ok(buf) = ring2.lock() {
                        let need = (input_sr as f32 * window_secs) as usize;
                        let len = buf.len();
                        let take = need.min(len);
                        v.reserve_exact(take);
                        // take from back
                        let start = len.saturating_sub(take);
                        for (i, sample) in buf.iter().enumerate() {
                            if i >= start { v.push(*sample); }
                        }
                        (v, len)
                    } else { (Vec::new(), 0) }
                };

                // Level every 200ms
                if last_level_emit.elapsed() >= level_interval {
                    if !buf_vec.is_empty() {
                        let n = (input_sr as f32 * 0.2) as usize;
                        let n = n.min(buf_vec.len());
                        let slice = &buf_vec[buf_vec.len()-n..];
                        let mut peak = 0f32; let mut sum2 = 0f64;
                        for &s in slice { let a = s.abs(); if a > peak { peak = a; } sum2 += (s as f64)*(s as f64); }
                        let rms = ((sum2 / (slice.len().max(1) as f64)) as f32).sqrt();
                        let _ = app_clone.emit_all("realtime-level", serde_json::json!({"peak": peak, "rms": rms}));
                    }
                    last_level_emit = Instant::now();
                }

                // Recognize every 1s if we have enough samples
                if last_recog.elapsed() >= recog_interval {
                    last_recog = Instant::now();
                    if !buf_vec.is_empty() {
                        // Resample to 16kHz
                        let input_rate = input_sr as f64;
                        let target_rate = 16000.0f64;
                        let window = buf_vec;
                        let resampled = match resample_mono(window, input_rate, target_rate) {
                            Ok(x) => x,
                            Err(e) => {
                                let _ = app_clone.emit_all("realtime-status", serde_json::json!({
                                    "phase": "error",
                                    "message": format!("リサンプル失敗: {}", e)
                                }));
                                continue;
                            }
                        };

                        // Transcribe (plain text) with possible language override
                        let text = match language_override.as_deref().and_then(|s| {
                            let s = s.trim();
                            if s.is_empty() || s.eq_ignore_ascii_case("auto") { None } else { Some(s) }
                        }) {
                            Some(lang) => engine.transcribe_with_language(&resampled, Some(lang)),
                            None => engine.transcribe(&resampled),
                        };
                        let text = match text {
                            Ok(t) => t.trim().to_string(),
                            Err(e) => {
                                let _ = app_clone.emit_all("realtime-status", serde_json::json!({
                                    "phase": "error",
                                    "message": format!("認識失敗: {}", e)
                                }));
                                String::new()
                            }
                        };

                        if !text.is_empty() {
                            let _ = app_clone.emit_all("realtime-text", serde_json::json!({"kind":"partial","text": text}));
                            prev_partial = text;
                            step += 1;
                            if step % 3 == 0 { // 簡易に3秒ごと確定
                                let _ = app_clone.emit_all("realtime-text", serde_json::json!({"kind":"final","text": prev_partial}));
                                prev_partial.clear();
                            }
                        }
                    }
                }

                std::thread::sleep(Duration::from_millis(20));
            }

            // flush last partial
            if !prev_partial.is_empty() {
                let _ = app_clone.emit_all("realtime-text", serde_json::json!({"kind":"final","text": prev_partial}));
            }

            let _ = app_clone.emit_all("realtime-status", serde_json::json!({ "phase": "stopped" }));
        }));

        Ok(())
    }

    pub fn stop(&mut self, app: tauri::AppHandle) -> Result<()> {
        if !self.running.load(Ordering::SeqCst) { return Ok(()); }
        self.running.store(false, Ordering::SeqCst);
        if let Some(h) = self.recog_thread.take() { let _ = h.join(); }
        self.emit_status(&app, "stopped", None);
        Ok(())
    }

    fn emit_status(&self, app: &tauri::AppHandle, phase: &str, message: Option<String>) {
        let _ = app.emit_all("realtime-status", serde_json::json!({
            "phase": phase,
            "message": message
        }));
    }
}

fn build_stream_f32(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    ring: Arc<Mutex<VecDeque<f32>>>,
    running: Arc<AtomicBool>,
) -> Result<cpal::Stream> {
    let cfg = config.clone();
    let err_fn = |e| eprintln!("cpal stream error: {e}");
    let input_data_fn = move |data: &[f32], _: &cpal::InputCallbackInfo| {
        if !running.load(Ordering::SeqCst) { return; }
        let mut mono_buf: Vec<f32> = Vec::with_capacity(data.len() / channels + 1);
        for frame in data.chunks_exact(channels) {
            let mut acc = 0.0f32;
            for c in 0..channels { acc += frame[c]; }
            mono_buf.push(acc / (channels as f32));
        }
        if let Ok(mut rb) = ring.lock() {
            for s in mono_buf { rb.push_back(s); }
            let max_len = (cfg.sample_rate.0 as usize) * 30;
            while rb.len() > max_len { rb.pop_front(); }
        }
    };
    let stream = device.build_input_stream(config, input_data_fn, err_fn, None)?;
    Ok(stream)
}

fn build_stream_i16(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    ring: Arc<Mutex<VecDeque<f32>>>,
    running: Arc<AtomicBool>,
) -> Result<cpal::Stream> {
    let cfg = config.clone();
    let err_fn = |e| eprintln!("cpal stream error: {e}");
    let input_data_fn = move |data: &[i16], _: &cpal::InputCallbackInfo| {
        if !running.load(Ordering::SeqCst) { return; }
        let mut mono_buf: Vec<f32> = Vec::with_capacity(data.len() / channels + 1);
        for frame in data.chunks_exact(channels) {
            let mut acc = 0.0f32;
            for c in 0..channels {
                acc += (frame[c] as f32) / (i16::MAX as f32);
            }
            mono_buf.push(acc / (channels as f32));
        }
        if let Ok(mut rb) = ring.lock() {
            for s in mono_buf { rb.push_back(s); }
            let max_len = (cfg.sample_rate.0 as usize) * 30;
            while rb.len() > max_len { rb.pop_front(); }
        }
    };
    let stream = device.build_input_stream(config, input_data_fn, err_fn, None)?;
    Ok(stream)
}

fn build_stream_u16(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: usize,
    ring: Arc<Mutex<VecDeque<f32>>>,
    running: Arc<AtomicBool>,
) -> Result<cpal::Stream> {
    let cfg = config.clone();
    let err_fn = |e| eprintln!("cpal stream error: {e}");
    let input_data_fn = move |data: &[u16], _: &cpal::InputCallbackInfo| {
        if !running.load(Ordering::SeqCst) { return; }
        let mut mono_buf: Vec<f32> = Vec::with_capacity(data.len() / channels + 1);
        for frame in data.chunks_exact(channels) {
            let mut acc = 0.0f32;
            for c in 0..channels {
                let s = frame[c] as f32;
                // Map 0..=65535 to roughly -1.0..=1.0
                acc += (s / (u16::MAX as f32)) * 2.0 - 1.0;
            }
            mono_buf.push(acc / (channels as f32));
        }
        if let Ok(mut rb) = ring.lock() {
            for s in mono_buf { rb.push_back(s); }
            let max_len = (cfg.sample_rate.0 as usize) * 30;
            while rb.len() > max_len { rb.pop_front(); }
        }
    };
    let stream = device.build_input_stream(config, input_data_fn, err_fn, None)?;
    Ok(stream)
}

fn resample_mono(samples: Vec<f32>, input_rate: f64, output_rate: f64) -> Result<Vec<f32>> {
    if (input_rate - output_rate).abs() < 1.0 { return Ok(samples); }
    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };
    let mut resampler = SincFixedIn::<f32>::new(
        output_rate / input_rate,
        2.0,
        params,
        samples.len(),
        1,
    )?;
    let input_channels = vec![samples];
    let output_channels = resampler.process(&input_channels, None)?;
    Ok(output_channels[0].clone())
}

// ===== Tauri commands =====
#[tauri::command]
pub fn realtime_start(app: tauri::AppHandle, device: Option<String>, language: Option<String>, threads: Option<usize>) -> Result<(), String> {
    let mut mgr = manager().lock().map_err(|_| "manager lock failed")?;
    mgr.start(app, device, language, threads).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn realtime_stop(app: tauri::AppHandle) -> Result<(), String> {
    let mut mgr = manager().lock().map_err(|_| "manager lock failed")?;
    mgr.stop(app).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn realtime_status() -> Result<RealtimeStatus, String> {
    let mgr = manager().lock().map_err(|_| "manager lock failed")?;
    Ok(mgr.status())
}

/// 入力デバイス名の一覧を返す（既定デバイスは先頭に含めず、UI側で「既定」を用意する想定）。
#[tauri::command]
pub fn list_input_devices() -> Result<Vec<String>, String> {
    let host = cpal::default_host();
    let mut names: Vec<String> = Vec::new();
    match host.input_devices() {
        Ok(devs) => {
            for d in devs {
                if let Ok(name) = d.name() { names.push(name); }
            }
            names.sort();
            names.dedup();
            Ok(names)
        }
        Err(e) => Err(format!("入力デバイス列挙に失敗しました: {}", e)),
    }
}

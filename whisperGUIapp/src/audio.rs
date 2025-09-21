//! 音声ファイルの読み込み・メタデータ取得・リサンプリングなど、
//! Whisper 前処理に関わる機能をまとめたモジュール。
//! - 任意のコンテナ/コーデックを Symphonia でデコード
//! - 複数チャネルをモノラルへ集約
//! - 16kHz など指定サンプリングレートへリサンプリング
//! - WAV への簡易書き出し（プレビュー用）

use crate::config::Config;
use anyhow::Result;
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use std::fs::File;
use std::path::Path;
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

/// 音声ファイルの基本メタデータ。
pub struct AudioMetadata {
    pub duration_seconds: f32,
    pub sample_rate: u32,
}

/// 音声処理の中核クラス。コンフィグに基づいてデコードやリサンプリングを行う。
pub struct AudioProcessor {
    config: Config,
}

impl AudioProcessor {
    /// 構成を取り込み、プロセッサを作成。
    pub fn new(config: &Config) -> Result<Self> {
        Ok(Self {
            config: config.clone(),
        })
    }

    /// デコードせずに長さやサンプリングレートなどを概算取得する。
    pub fn probe_metadata(&self, file_path: &str) -> Result<AudioMetadata> {
        let path = Path::new(file_path);
        if !path.exists() {
            return Err(anyhow::anyhow!(
                "音声ファイルが見つかりません: {}",
                file_path
            ));
        }

        let file = File::open(path)?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
            hint.with_extension(extension);
        }

        let meta_opts: MetadataOptions = Default::default();
        let fmt_opts: FormatOptions = Default::default();

        let probed = symphonia::default::get_probe().format(&hint, mss, &fmt_opts, &meta_opts)?;

        let mut format = probed.format;

        let track = format
            .tracks()
            .iter()
            .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
            .ok_or_else(|| anyhow::anyhow!("音声トラックが見つかりません"))?;

        let track_id = track.id;
        let sample_rate = track
            .codec_params
            .sample_rate
            .ok_or_else(|| anyhow::anyhow!("サンプリングレートが取得できません"))?
            as u32;
        let time_base = track.codec_params.time_base;

        let mut total_duration = 0u64;
        let mut total_frames = 0u64;

        // 各パケットを走査して総フレーム数/総durationを算出
        loop {
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(symphonia::core::errors::Error::ResetRequired) => {
                    break;
                }
                Err(symphonia::core::errors::Error::IoError(ref err))
                    if err.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break;
                }
                Err(err) => {
                    return Err(anyhow::anyhow!("パケット読み込みエラー: {}", err));
                }
            };

            if packet.track_id() != track_id {
                continue;
            }

            total_duration = total_duration.saturating_add(packet.dur());
            total_frames = total_frames.saturating_add(packet.block_dur());
        }

        if total_duration == 0 && total_frames == 0 {
            return Err(anyhow::anyhow!("音声データが空です"));
        }

        // time_base があればそれを使用、なければフレーム数/サンプルレートで概算
        let duration_seconds = if let Some(time_base) = time_base {
            let time = time_base.calc_time(total_duration);
            (time.seconds as f64 + time.frac) as f32
        } else if total_frames > 0 {
            (total_frames as f64 / sample_rate as f64) as f32
        } else {
            0.0
        };

        Ok(AudioMetadata {
            duration_seconds,
            sample_rate,
        })
    }

    /// 音声ファイルを読み込み、モノラル f32 波形に変換して返す（必要に応じて指定レートへリサンプリング）。
    pub fn load_audio_file(&mut self, file_path: &str) -> Result<Vec<f32>> {
        let path = Path::new(file_path);
        if !path.exists() {
            return Err(anyhow::anyhow!(
                "音声ファイルが見つかりません: {}",
                file_path
            ));
        }

        // ファイルを開く
        let file = File::open(path)?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        // フォーマットヒントを設定
        let mut hint = Hint::new();
        if let Some(extension) = path.extension() {
            if let Some(extension_str) = extension.to_str() {
                hint.with_extension(extension_str);
            }
        }

        // フォーマットを推定
        let meta_opts: MetadataOptions = Default::default();
        let fmt_opts: FormatOptions = Default::default();

        let probed = symphonia::default::get_probe().format(&hint, mss, &fmt_opts, &meta_opts)?;

        let mut format = probed.format;

        // 最初のオーディオトラックを見つける
        let (track_id, codec_params) = {
            let track = format
                .tracks()
                .iter()
                .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
                .ok_or_else(|| anyhow::anyhow!("音声トラックが見つかりません"))?;

            (track.id, track.codec_params.clone())
        };

        // デコーダーを作成
        let dec_opts: DecoderOptions = Default::default();
        let mut decoder = symphonia::default::get_codecs().make(&codec_params, &dec_opts)?;

        let mut samples = Vec::new();

        // パケットをデコード
        loop {
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(symphonia::core::errors::Error::ResetRequired) => {
                    // リセットが必要な場合
                    break;
                }
                Err(symphonia::core::errors::Error::IoError(ref err))
                    if err.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    // ファイル終端
                    break;
                }
                Err(err) => return Err(anyhow::anyhow!("パケット読み込みエラー: {}", err)),
            };

            // 正しいトラックのパケットのみを処理
            if packet.track_id() != track_id {
                continue;
            }

            match decoder.decode(&packet) {
                Ok(audio_buf) => {
                    // 各フレームで全チャネルを平均し、モノラルに変換して蓄積
                    self.extract_samples_from_buffer(&audio_buf, &mut samples)?;
                }
                Err(symphonia::core::errors::Error::IoError(ref err))
                    if err.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    // デコード終了
                    break;
                }
                Err(err) => return Err(anyhow::anyhow!("デコードエラー: {}", err)),
            }
        }

        if samples.is_empty() {
            return Err(anyhow::anyhow!("音声データが空です"));
        }

        // サンプリングレートを取得（デコーダが提供する値を優先）
        let original_sample_rate = decoder
            .codec_params()
            .sample_rate
            .or(codec_params.sample_rate)
            .ok_or_else(|| anyhow::anyhow!("サンプリングレートが取得できません"))? as f64;

        // 既にモノラルへ変換済み
        let mono_samples = samples;

        // 16kHzなど設定レートにリサンプル
        let target_sample_rate = self.config.audio.sample_rate as f64;
        let resampled = if (original_sample_rate - target_sample_rate).abs() > 1.0 {
            self.resample_audio(mono_samples, original_sample_rate, target_sample_rate)?
        } else {
            mono_samples
        };

        println!(
            "音声ファイル読み込み完了: {} samples, {}Hz -> {}Hz",
            resampled.len(),
            original_sample_rate,
            target_sample_rate
        );

        Ok(resampled)
    }

    /// 入力ファイルを読み込み、モノラル16bit PCM WAV で保存（プレビュー用）。
    pub fn decode_to_wav_file(&mut self, src_path: &str, dst_path: &str) -> Result<()> {
        let samples_f32 = self.load_audio_file(src_path)?;
        let sr = self.config.audio.sample_rate as u32;
        write_wav_mono_16(dst_path, sr, &samples_f32)?;
        Ok(())
    }

    /// デコード済みバッファから f32 モノラル波形を抽出（各サンプル形式を正規化）。
    fn extract_samples_from_buffer(
        &self,
        audio_buf: &AudioBufferRef,
        samples: &mut Vec<f32>,
    ) -> Result<()> {
        match audio_buf {
            AudioBufferRef::F32(buf) => {
                let ch = buf.spec().channels.count();
                let frames = buf.frames();
                for i in 0..frames {
                    let mut sum = 0.0f32;
                    for c in 0..ch {
                        sum += buf.chan(c)[i];
                    }
                    samples.push(sum / ch as f32);
                }
            }
            AudioBufferRef::S32(buf) => {
                let ch = buf.spec().channels.count();
                let frames = buf.frames();
                for i in 0..frames {
                    let mut sum = 0.0f32;
                    for c in 0..ch {
                        sum += buf.chan(c)[i] as f32 / i32::MAX as f32;
                    }
                    samples.push(sum / ch as f32);
                }
            }
            AudioBufferRef::S16(buf) => {
                let ch = buf.spec().channels.count();
                let frames = buf.frames();
                for i in 0..frames {
                    let mut sum = 0.0f32;
                    for c in 0..ch {
                        sum += buf.chan(c)[i] as f32 / i16::MAX as f32;
                    }
                    samples.push(sum / ch as f32);
                }
            }
            _ => return Err(anyhow::anyhow!("サポートされていない音声フォーマットです")),
        }
        Ok(())
    }

    /// 多チャネル波形を単純平均でモノラル化。
    fn convert_to_mono(&self, samples: Vec<f32>, channels: usize) -> Vec<f32> {
        if channels == 1 {
            return samples;
        }

        let mono_len = samples.len() / channels;
        let mut mono_samples = Vec::with_capacity(mono_len);

        for i in 0..mono_len {
            let mut sum = 0.0;
            for ch in 0..channels {
                sum += samples[i * channels + ch];
            }
            mono_samples.push(sum / channels as f32);
        }

        mono_samples
    }

    /// SincFixed（rubato）で音質と負荷のバランスを取りつつリサンプリング。
    fn resample_audio(
        &self,
        samples: Vec<f32>,
        input_rate: f64,
        output_rate: f64,
    ) -> Result<Vec<f32>> {
        if (input_rate - output_rate).abs() < 1.0 {
            return Ok(samples);
        }

        // SincFixed リサンプラーを作成（音質と負荷のバランスを重視）
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
            1, // モノラル
        )?;

        // チャンネルごとにベクターを準備（モノラルなので1ch）
        let input_channels = vec![samples];

        // リサンプリング実行
        let output_channels = resampler.process(&input_channels, None)?;

        Ok(output_channels[0].clone())
    }
}

/// 非依存ライブラリでの簡易WAV書き出し（モノラル16bit）。
fn write_wav_mono_16(path: &str, sample_rate: u32, samples: &[f32]) -> Result<()> {
    use std::fs::File;
    use std::io::{Seek, SeekFrom, Write};

    let mut file = File::create(path)?;

    // RIFF header
    file.write_all(b"RIFF")?;
    file.write_all(&[0u8; 4])?; // placeholder for chunk size
    file.write_all(b"WAVE")?;

    // fmt chunk
    file.write_all(b"fmt ")?;
    file.write_all(&16u32.to_le_bytes())?; // PCM fmt chunk size
    file.write_all(&1u16.to_le_bytes())?; // Audio format = 1 (PCM)
    file.write_all(&1u16.to_le_bytes())?; // Channels = 1
    file.write_all(&sample_rate.to_le_bytes())?; // Sample rate
    let byte_rate: u32 = sample_rate * 1 * 2; // sr * channels * bytes_per_sample
    file.write_all(&byte_rate.to_le_bytes())?;
    let block_align: u16 = 1 * 2; // channels * bytes_per_sample
    file.write_all(&block_align.to_le_bytes())?;
    file.write_all(&16u16.to_le_bytes())?; // bits per sample

    // data chunk
    file.write_all(b"data")?;
    let data_bytes: u32 = (samples.len() as u32) * 2; // i16
    file.write_all(&data_bytes.to_le_bytes())?;

    // samples
    for &s in samples.iter() {
        let v = (s.max(-1.0).min(1.0) * i16::MAX as f32) as i16;
        file.write_all(&v.to_le_bytes())?;
    }

    // finalize RIFF chunk size = 4 ("WAVE") + (8+fmt) + (8+data)
    let riff_size: u32 = 4 + (8 + 16) + (8 + data_bytes);
    file.seek(SeekFrom::Start(4))?;
    file.write_all(&riff_size.to_le_bytes())?;

    Ok(())
}

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
use tempfile::{NamedTempFile, TempDir};
use std::io::Write;

#[derive(Debug, Clone)]
pub struct AudioMetadata {
    pub duration_seconds: f32,
    pub sample_rate: u32,
    pub channels: u16,
    pub file_size_bytes: u64,
    pub format: String,
}

#[derive(Debug)]
pub struct ProcessedAudio {
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub duration_ms: u64,
    pub original_metadata: AudioMetadata,
}

pub struct AudioProcessor {
    config: Config,
    temp_dir: TempDir,
}

impl AudioProcessor {
    pub fn new(config: &Config) -> Result<Self> {
        let temp_dir = TempDir::new_in(&config.paths.temp_dir)?;

        Ok(Self {
            config: config.clone(),
            temp_dir,
        })
    }

    /// 音声ファイルのメタデータを取得
    pub fn probe_metadata<P: AsRef<Path>>(&self, file_path: P) -> Result<AudioMetadata> {
        let path = file_path.as_ref();
        if !path.exists() {
            return Err(anyhow::anyhow!(
                "音声ファイルが見つかりません: {}",
                path.display()
            ));
        }

        let file_size_bytes = std::fs::metadata(path)?.len();
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

        let channels = track
            .codec_params
            .channels
            .map(|ch| ch.count() as u16)
            .unwrap_or(1);

        let time_base = track.codec_params.time_base;

        let mut total_duration = 0u64;
        let mut total_frames = 0u64;

        loop {
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(symphonia::core::errors::Error::ResetRequired) => break,
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

        let duration_seconds = if let Some(time_base) = time_base {
            let time = time_base.calc_time(total_duration);
            (time.seconds as f64 + time.frac) as f32
        } else if total_frames > 0 {
            (total_frames as f64 / sample_rate as f64) as f32
        } else {
            0.0
        };

        let format_name = path
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("unknown")
            .to_lowercase();

        Ok(AudioMetadata {
            duration_seconds,
            sample_rate,
            channels,
            file_size_bytes,
            format: format_name,
        })
    }

    /// バイト配列から一時ファイルを作成し、音声データを処理
    pub fn process_audio_from_bytes(&mut self, audio_bytes: &[u8], filename: &str) -> Result<ProcessedAudio> {
        // 一時ファイルを作成
        let temp_file = self.create_temp_file_from_bytes(audio_bytes, filename)?;
        let temp_path = temp_file.path();

        self.process_audio_file(temp_path)
    }

    /// 音声ファイルを処理してWhisper用のf32サンプルに変換
    pub fn process_audio_file<P: AsRef<Path>>(&mut self, file_path: P) -> Result<ProcessedAudio> {
        let path = file_path.as_ref();

        // メタデータを取得
        let metadata = self.probe_metadata(path)?;

        // 音声データを読み込み
        let samples = self.load_audio_file(path)?;

        // 期間を計算
        let duration_ms = (samples.len() as f64 / self.config.audio.sample_rate as f64 * 1000.0) as u64;

        Ok(ProcessedAudio {
            samples,
            sample_rate: self.config.audio.sample_rate,
            duration_ms,
            original_metadata: metadata,
        })
    }

    /// 音声ファイルをf32サンプル配列として読み込み
    pub fn load_audio_file<P: AsRef<Path>>(&mut self, file_path: P) -> Result<Vec<f32>> {
        let path = file_path.as_ref();
        if !path.exists() {
            return Err(anyhow::anyhow!(
                "音声ファイルが見つかりません: {}",
                path.display()
            ));
        }

        let file = File::open(path)?;
        let mss = MediaSourceStream::new(Box::new(file), Default::default());

        let mut hint = Hint::new();
        if let Some(extension) = path.extension() {
            if let Some(extension_str) = extension.to_str() {
                hint.with_extension(extension_str);
            }
        }

        let meta_opts: MetadataOptions = Default::default();
        let fmt_opts: FormatOptions = Default::default();

        let probed = symphonia::default::get_probe().format(&hint, mss, &fmt_opts, &meta_opts)?;
        let mut format = probed.format;

        let (track_id, codec_params) = {
            let track = format
                .tracks()
                .iter()
                .find(|t| t.codec_params.codec != CODEC_TYPE_NULL)
                .ok_or_else(|| anyhow::anyhow!("音声トラックが見つかりません"))?;

            (track.id, track.codec_params.clone())
        };

        let dec_opts: DecoderOptions = Default::default();
        let mut decoder = symphonia::default::get_codecs().make(&codec_params, &dec_opts)?;

        let mut samples = Vec::new();

        loop {
            let packet = match format.next_packet() {
                Ok(packet) => packet,
                Err(symphonia::core::errors::Error::ResetRequired) => break,
                Err(symphonia::core::errors::Error::IoError(ref err))
                    if err.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break;
                }
                Err(err) => return Err(anyhow::anyhow!("パケット読み込みエラー: {}", err)),
            };

            if packet.track_id() != track_id {
                continue;
            }

            match decoder.decode(&packet) {
                Ok(audio_buf) => {
                    self.extract_samples_from_buffer(&audio_buf, &mut samples)?;
                }
                Err(symphonia::core::errors::Error::IoError(ref err))
                    if err.kind() == std::io::ErrorKind::UnexpectedEof =>
                {
                    break;
                }
                Err(err) => return Err(anyhow::anyhow!("デコードエラー: {}", err)),
            }
        }

        if samples.is_empty() {
            return Err(anyhow::anyhow!("音声データが空です"));
        }

        let original_sample_rate = decoder
            .codec_params()
            .sample_rate
            .or(codec_params.sample_rate)
            .ok_or_else(|| anyhow::anyhow!("サンプリングレートが取得できません"))? as f64;

        let target_sample_rate = self.config.audio.sample_rate as f64;
        let resampled = if (original_sample_rate - target_sample_rate).abs() > 1.0 {
            self.resample_audio(samples, original_sample_rate, target_sample_rate)?
        } else {
            samples
        };

        Ok(resampled)
    }

    /// バイト配列から一時ファイルを作成
    pub fn create_temp_file_from_bytes(&self, bytes: &[u8], filename: &str) -> Result<NamedTempFile> {
        let extension = Path::new(filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("bin");

        let mut temp_file = NamedTempFile::with_suffix_in(
            &format!(".{}", extension),
            &self.temp_dir
        )?;

        std::io::Write::write_all(&mut temp_file, bytes)?;
        temp_file.flush()?;

        Ok(temp_file)
    }

    /// サポートされているファイル形式かチェック
    pub fn is_supported_format(&self, filename: &str) -> bool {
        let extension = Path::new(filename)
            .extension()
            .and_then(|ext| ext.to_str())
            .map(|s| s.to_lowercase())
            .unwrap_or_default();

        self.config.audio.supported_formats.contains(&extension)
    }

    /// ファイルサイズ制限をチェック
    pub fn validate_file_size(&self, size_bytes: usize) -> Result<()> {
        let max_size = self.config.max_file_size_bytes();
        if size_bytes > max_size {
            return Err(anyhow::anyhow!(
                "ファイルサイズが制限を超えています: {}MB > {}MB",
                size_bytes / (1024 * 1024),
                max_size / (1024 * 1024)
            ));
        }
        Ok(())
    }

    /// 音声の長さ制限をチェック
    pub fn validate_audio_duration(&self, metadata: &AudioMetadata) -> Result<()> {
        let max_duration_minutes = self.config.limits.max_audio_duration_minutes as f32;
        let duration_minutes = metadata.duration_seconds / 60.0;

        if duration_minutes > max_duration_minutes {
            return Err(anyhow::anyhow!(
                "音声ファイルが長すぎます: {:.1}分 > {:.1}分",
                duration_minutes,
                max_duration_minutes
            ));
        }
        Ok(())
    }

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

    fn resample_audio(
        &self,
        samples: Vec<f32>,
        input_rate: f64,
        output_rate: f64,
    ) -> Result<Vec<f32>> {
        if (input_rate - output_rate).abs() < 1.0 {
            return Ok(samples);
        }

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

        let input_channels = vec![samples];
        let output_channels = resampler.process(&input_channels, None)?;

        Ok(output_channels[0].clone())
    }
}

impl Drop for AudioProcessor {
    fn drop(&mut self) {
        // 一時ディレクトリは自動的にクリーンアップされます
    }
}

// =============================================================================
// Utility Functions
// =============================================================================

/// 音声ファイルの形式を検出
pub fn detect_audio_format<P: AsRef<Path>>(file_path: P) -> Result<String> {
    let path = file_path.as_ref();

    if let Some(extension) = path.extension().and_then(|ext| ext.to_str()) {
        Ok(extension.to_lowercase())
    } else {
        // ファイルの先頭バイトから形式を推測
        let mut file = File::open(path)?;
        let mut buffer = [0u8; 12];
        std::io::Read::read_exact(&mut file, &mut buffer)?;

        if &buffer[0..4] == b"RIFF" && &buffer[8..12] == b"WAVE" {
            Ok("wav".to_string())
        } else if &buffer[0..3] == b"ID3" || &buffer[0..2] == [0xFF, 0xFB] {
            Ok("mp3".to_string())
        } else if &buffer[4..8] == b"ftyp" {
            Ok("m4a".to_string())
        } else if &buffer[0..4] == b"fLaC" {
            Ok("flac".to_string())
        } else if &buffer[0..4] == b"OggS" {
            Ok("ogg".to_string())
        } else {
            Err(anyhow::anyhow!("不明な音声フォーマットです"))
        }
    }
}

/// ファイルサイズを人間が読みやすい形式で表示
pub fn format_file_size(bytes: u64) -> String {
    const UNITS: &[&str] = &["B", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit_index = 0;

    while size >= 1024.0 && unit_index < UNITS.len() - 1 {
        size /= 1024.0;
        unit_index += 1;
    }

    if unit_index == 0 {
        format!("{} {}", size as u64, UNITS[unit_index])
    } else {
        format!("{:.1} {}", size, UNITS[unit_index])
    }
}
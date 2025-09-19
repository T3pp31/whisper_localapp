use crate::config::Config;
use anyhow::Result;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Device, Stream};
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use std::fs::File;
use std::path::Path;
use std::sync::{Arc, Mutex};
use symphonia::core::audio::{AudioBufferRef, Signal};
use symphonia::core::codecs::{DecoderOptions, CODEC_TYPE_NULL};
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

pub struct AudioProcessor {
    config: Config,
    recording_device: Option<Device>,
    recording_stream: Option<Stream>,
    recorded_samples: Arc<Mutex<Vec<f32>>>,
}

impl AudioProcessor {
    pub fn new(config: &Config) -> Result<Self> {
        Ok(Self {
            config: config.clone(),
            recording_device: None,
            recording_stream: None,
            recorded_samples: Arc::new(Mutex::new(Vec::new())),
        })
    }

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

        // サンプリングレートを取得
        let original_sample_rate = codec_params
            .sample_rate
            .ok_or_else(|| anyhow::anyhow!("サンプリングレートが取得できません"))?
            as f64;

        // チャンネル数を取得
        let channels = codec_params
            .channels
            .ok_or_else(|| anyhow::anyhow!("チャンネル数が取得できません"))?
            .count();

        // モノラルに変換
        let mono_samples = self.convert_to_mono(samples, channels);

        // 16kHzにリサンプル
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

    fn extract_samples_from_buffer(
        &self,
        audio_buf: &AudioBufferRef,
        samples: &mut Vec<f32>,
    ) -> Result<()> {
        match audio_buf {
            AudioBufferRef::F32(buf) => {
                for &sample in buf.chan(0) {
                    samples.push(sample);
                }
                for ch in 1..buf.spec().channels.count() {
                    for (i, &sample) in buf.chan(ch).iter().enumerate() {
                        if i < samples.len() {
                            samples[i] += sample;
                        }
                    }
                }
            }
            AudioBufferRef::S32(buf) => {
                for &sample in buf.chan(0) {
                    samples.push(sample as f32 / i32::MAX as f32);
                }
                for ch in 1..buf.spec().channels.count() {
                    for (i, &sample) in buf.chan(ch).iter().enumerate() {
                        if i < samples.len() {
                            samples[i] += sample as f32 / i32::MAX as f32;
                        }
                    }
                }
            }
            AudioBufferRef::S16(buf) => {
                for &sample in buf.chan(0) {
                    samples.push(sample as f32 / i16::MAX as f32);
                }
                for ch in 1..buf.spec().channels.count() {
                    for (i, &sample) in buf.chan(ch).iter().enumerate() {
                        if i < samples.len() {
                            samples[i] += sample as f32 / i16::MAX as f32;
                        }
                    }
                }
            }
            _ => {
                return Err(anyhow::anyhow!("サポートされていない音声フォーマットです"));
            }
        }
        Ok(())
    }

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

    fn resample_audio(
        &self,
        samples: Vec<f32>,
        input_rate: f64,
        output_rate: f64,
    ) -> Result<Vec<f32>> {
        if (input_rate - output_rate).abs() < 1.0 {
            return Ok(samples);
        }

        // SincFixed リサンプラーを作成
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

        // チャンネルごとにベクターを準備
        let input_channels = vec![samples];

        // リサンプリング実行
        let output_channels = resampler.process(&input_channels, None)?;

        Ok(output_channels[0].clone())
    }

    pub fn start_recording(&mut self) -> Result<()> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .ok_or_else(|| anyhow::anyhow!("デフォルト入力デバイスが見つかりません"))?;

        let config = device.default_input_config()?;

        println!("録音デバイス: {}", device.name()?);
        println!("録音設定: {:?}", config);

        let channels = config.channels();

        let recorded_samples = self.recorded_samples.clone();

        let stream = match config.sample_format() {
            cpal::SampleFormat::F32 => {
                device.build_input_stream(
                    &config.into(),
                    move |data: &[f32], _: &cpal::InputCallbackInfo| {
                        let mut samples = recorded_samples.lock().unwrap();

                        // モノラルに変換して追加
                        if channels == 1 {
                            samples.extend_from_slice(data);
                        } else {
                            for chunk in data.chunks(channels as usize) {
                                let mono_sample = chunk.iter().sum::<f32>() / channels as f32;
                                samples.push(mono_sample);
                            }
                        }
                    },
                    |err| eprintln!("録音エラー: {}", err),
                    None,
                )?
            }
            cpal::SampleFormat::I16 => device.build_input_stream(
                &config.into(),
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    let mut samples = recorded_samples.lock().unwrap();

                    if channels == 1 {
                        for &sample in data {
                            samples.push(sample as f32 / i16::MAX as f32);
                        }
                    } else {
                        for chunk in data.chunks(channels as usize) {
                            let mono_sample: f32 = chunk
                                .iter()
                                .map(|&s| s as f32 / i16::MAX as f32)
                                .sum::<f32>()
                                / channels as f32;
                            samples.push(mono_sample);
                        }
                    }
                },
                |err| eprintln!("録音エラー: {}", err),
                None,
            )?,
            format => {
                return Err(anyhow::anyhow!(
                    "サポートされていない音声フォーマット: {:?}",
                    format
                ));
            }
        };

        stream.play()?;

        self.recording_device = Some(device);
        self.recording_stream = Some(stream);

        println!("録音を開始しました");
        Ok(())
    }

    pub fn stop_recording(&mut self) -> Result<Vec<f32>> {
        if let Some(stream) = self.recording_stream.take() {
            stream.pause()?;
            drop(stream);
        }

        self.recording_device = None;

        let mut samples = self.recorded_samples.lock().unwrap();
        let recorded_data = samples.clone();
        samples.clear();

        println!("録音を停止しました。{} samples収録", recorded_data.len());

        // 16kHzにリサンプル（必要に応じて）
        // 現在は簡略化のため、そのまま返す
        Ok(recorded_data)
    }

    pub fn is_recording(&self) -> bool {
        self.recording_stream.is_some()
    }
}

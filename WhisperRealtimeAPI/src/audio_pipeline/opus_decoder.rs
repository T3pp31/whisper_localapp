use audiopus::coder::Decoder as OpusDecoder;
use audiopus::{Channels, SampleRate};
use bytes::Bytes;
use tracing::debug;

/// Opusデコーダーラッパー
pub struct AudioOpusDecoder {
    decoder: OpusDecoder,
    sample_rate: u32,
    channels: usize,
}

impl AudioOpusDecoder {
    /// 新しいOpusデコーダーを作成
    pub fn new(sample_rate: u32, channels: usize) -> Result<Self, String> {
        let opus_channels = match channels {
            1 => Channels::Mono,
            2 => Channels::Stereo,
            _ => return Err(format!("未対応のチャネル数: {}", channels)),
        };

        let opus_sample_rate = match sample_rate {
            8000 => SampleRate::Hz8000,
            12000 => SampleRate::Hz12000,
            16000 => SampleRate::Hz16000,
            24000 => SampleRate::Hz24000,
            48000 => SampleRate::Hz48000,
            _ => return Err(format!("未対応のサンプルレート: {}", sample_rate)),
        };

        let decoder = OpusDecoder::new(opus_sample_rate, opus_channels)
            .map_err(|e| format!("Opusデコーダー初期化失敗: {:?}", e))?;

        Ok(Self {
            decoder,
            sample_rate,
            channels,
        })
    }

    /// Opusパケットをデコード
    pub fn decode(&mut self, packet: &Bytes) -> Result<Vec<i16>, String> {
        // フレームサイズを計算（20ms想定）
        let frame_size = (self.sample_rate / 50) as usize;
        let mut output = vec![0i16; frame_size * self.channels];

        let decoded_samples = self
            .decoder
            .decode(Some(audiopus::packet::Packet::try_from(packet.as_ref()).unwrap()), audiopus::MutSignals::try_from(&mut output[..]).unwrap(), false)
            .map_err(|e| format!("Opusデコード失敗: {:?}", e))?;

        output.truncate(decoded_samples * self.channels);

        debug!(
            samples = decoded_samples,
            packet_size = packet.len(),
            "Opusデコード完了"
        );

        Ok(output)
    }

    /// パケット損失時のPLC（Packet Loss Concealment）
    pub fn decode_plc(&mut self) -> Result<Vec<i16>, String> {
        let frame_size = (self.sample_rate / 50) as usize;
        let mut output = vec![0i16; frame_size * self.channels];

        let decoded_samples = self
            .decoder
            .decode(None, audiopus::MutSignals::try_from(&mut output[..]).unwrap(), false)
            .map_err(|e| format!("PLC処理失敗: {:?}", e))?;

        output.truncate(decoded_samples * self.channels);

        debug!(samples = decoded_samples, "PLC処理完了");

        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_opus_decoder_creation() {
        let decoder = AudioOpusDecoder::new(48000, 2);
        assert!(decoder.is_ok());
    }

    #[test]
    fn test_invalid_sample_rate() {
        let decoder = AudioOpusDecoder::new(44100, 2);
        assert!(decoder.is_err());
    }

    #[test]
    fn test_invalid_channels() {
        let decoder = AudioOpusDecoder::new(48000, 3);
        assert!(decoder.is_err());
    }
}
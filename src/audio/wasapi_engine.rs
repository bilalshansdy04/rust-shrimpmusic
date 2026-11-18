use wasapi::{
    AudioClient, AudioRenderClient, Device, Direction, Handle, SampleType, StreamMode,
    WaveFormat,
};

use super::dsp::TargetSampleFormat;
use super::error::{AudioError, AudioResult};

pub struct WasapiStream {
    client: AudioClient,
    render_client: AudioRenderClient,
    event_handle: Handle,
    format: WaveFormat,
    target_sample_format: TargetSampleFormat,
    is_exclusive: bool,
    channels: usize,
    sample_rate: u32,
    bytes_per_sample: usize,
}

impl WasapiStream {
    pub fn open(
        device: &Device,
        desired_sample_rate: u32,
        desired_channels: usize,
        desired_bit_depth: Option<u32>,
        exclusive: bool,
    ) -> AudioResult<Self> {
        let mut client = device.get_iaudioclient()?;

        let (wave_fmt, target_sample_format, is_exclusive) = if exclusive {
            match Self::negotiate_exclusive_format(
                &client,
                desired_sample_rate,
                desired_channels,
                desired_bit_depth,
            ) {
                Ok((fmt, target_fmt)) => (fmt, target_fmt, true),
                Err(_) => {
                    let mix_fmt = client.get_mixformat()?;
                    let target_fmt = Self::sample_format_from_wavefmt(&mix_fmt)?;
                    (mix_fmt, target_fmt, false)
                }
            }
        } else {
            let mix_fmt = client.get_mixformat()?;
            let target_fmt = Self::sample_format_from_wavefmt(&mix_fmt)?;
            (mix_fmt, target_fmt, false)
        };

        let stream_mode = if is_exclusive {
            let (min_period, _) = client.get_device_period()?;
            StreamMode::EventsExclusive {
                period_hns: min_period,
            }
        } else {
            StreamMode::EventsShared {
                autoconvert: true,
                buffer_duration_hns: 0,
            }
        };

        client.initialize_client(&wave_fmt, Direction::Render, stream_mode, None)?;

        let event_handle = client.set_get_eventhandle()?;
        let render_client = client.get_audiorenderclient()?;
        let channels = wave_fmt.get_nchannels() as usize;
        let sample_rate = wave_fmt.get_samplespersec();
        let bytes_per_sample = (wave_fmt.get_bitspersample() / 8) as usize;

        client.start_stream()?;

        Ok(Self {
            client,
            render_client,
            event_handle,
            format: wave_fmt,
            target_sample_format,
            is_exclusive,
            channels,
            sample_rate,
            bytes_per_sample,
        })
    }

    fn negotiate_exclusive_format(
        client: &AudioClient,
        sample_rate: u32,
        channels: usize,
        bit_depth: Option<u32>,
    ) -> AudioResult<(WaveFormat, TargetSampleFormat)> {
        let bit_depth = bit_depth.unwrap_or(16);

        let candidates = [
            (bit_depth, bit_depth, SampleType::Int),
            (24, 24, SampleType::Int),
            (32, 24, SampleType::Int),
            (16, 16, SampleType::Int),
            (32, 32, SampleType::Float),
        ];

        for (bits_per_sample, valid_bits, sample_type) in candidates {
            let candidate_fmt = WaveFormat::new(
                bits_per_sample as usize,
                valid_bits as usize,
                sample_type,
                sample_rate as usize,
                channels,
                None,
            );

            if client.is_supported_exclusive_with_quirks(&candidate_fmt).is_ok() {
                let target_fmt = match (bits_per_sample, valid_bits, sample_type) {
                    (16, _, _) => TargetSampleFormat::Pcm16,
                    (24, 24, _) => TargetSampleFormat::Pcm24Packed,
                    (32, 24, _) => TargetSampleFormat::Pcm24In32,
                    (32, 32, SampleType::Int) => TargetSampleFormat::Pcm32,
                    (32, 32, SampleType::Float) => TargetSampleFormat::Float32,
                    _ => TargetSampleFormat::Pcm16,
                };
                return Ok((candidate_fmt, target_fmt));
            }
        }

        Err(AudioError::FormatNotSupported(format!(
            "{} Hz, {} channels in exclusive mode",
            sample_rate, channels
        )))
    }

    fn sample_format_from_wavefmt(fmt: &WaveFormat) -> AudioResult<TargetSampleFormat> {
        let bits = fmt.get_bitspersample();
        let sample_type = fmt.get_subformat().unwrap_or(SampleType::Float);

        match (bits, sample_type) {
            (16, _) => Ok(TargetSampleFormat::Pcm16),
            (24, _) => Ok(TargetSampleFormat::Pcm24Packed),
            (32, SampleType::Float) => Ok(TargetSampleFormat::Float32),
            (32, _) => Ok(TargetSampleFormat::Pcm32),
            _ => Ok(TargetSampleFormat::Float32),
        }
    }

    pub fn wave_format(&self) -> &WaveFormat {
        &self.format
    }

    pub fn is_exclusive(&self) -> bool {
        self.is_exclusive
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn channels(&self) -> usize {
        self.channels
    }

    pub fn target_sample_format(&self) -> TargetSampleFormat {
        match self.target_sample_format {
            TargetSampleFormat::Pcm16 => TargetSampleFormat::Pcm16,
            TargetSampleFormat::Pcm24Packed => TargetSampleFormat::Pcm24Packed,
            TargetSampleFormat::Pcm24In32 => TargetSampleFormat::Pcm24In32,
            TargetSampleFormat::Pcm32 => TargetSampleFormat::Pcm32,
            TargetSampleFormat::Float32 => TargetSampleFormat::Float32,
        }
    }

    pub fn wait_for_buffer(&self, timeout_ms: u32) -> AudioResult<()> {
        self.event_handle.wait_for_event(timeout_ms)?;
        Ok(())
    }

    pub fn available_frames(&self) -> AudioResult<usize> {
        let frames = self.client.get_available_space_in_frames()?;
        Ok(frames as usize)
    }

    pub fn write_bytes(&self, data: &[u8]) -> AudioResult<()> {
        self.render_client.write_to_device(data, None)?;
        Ok(())
    }

    pub fn bytes_per_frame(&self) -> usize {
        self.channels * self.bytes_per_sample
    }

    pub fn stop(&mut self) -> AudioResult<()> {
        let _ = self.client.stop_stream();
        let _ = self.client.reset_stream();
        Ok(())
    }
}

impl Drop for WasapiStream {
    fn drop(&mut self) {
        let _ = self.client.stop_stream();
        let _ = self.client.reset_stream();
    }
}

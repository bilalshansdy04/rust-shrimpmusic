use rubato::audioadapter_buffers::owned::InterleavedOwned;
use rubato::{Fft, FixedSync, Resampler};
use super::error::{AudioError, AudioResult};

pub struct DspProcessor {
    volume: f32,
    resampler: Option<ResamplerWrapper>,
}

struct ResamplerWrapper {
    resampler: Fft<f32>,
    channels: usize,
    chunk_size: usize,
    input_buffer: Vec<f32>,
}

impl DspProcessor {
    pub fn new() -> Self {
        Self {
            volume: 1.0,
            resampler: None,
        }
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }

    pub fn setup_resampler(
        &mut self,
        source_rate: u32,
        target_rate: u32,
        channels: usize,
    ) -> AudioResult<()> {
        if source_rate == target_rate {
            self.resampler = None;
            return Ok(());
        }

        let chunk_size = 1024;
        let resampler = Fft::<f32>::new(
            source_rate as usize,
            target_rate as usize,
            chunk_size,
            channels,
            FixedSync::Input,
        )
        .map_err(|e| AudioError::Resample(e.to_string()))?;

        self.resampler = Some(ResamplerWrapper {
            resampler,
            channels,
            chunk_size,
            input_buffer: Vec::with_capacity(chunk_size * channels * 2),
        });

        Ok(())
    }

    pub fn process_samples(&mut self, mut samples: Vec<f32>) -> AudioResult<Vec<f32>> {
        let gain = self.volume * self.volume;

        for s in samples.iter_mut() {
            let amplified = *s * gain;
            *s = amplified.clamp(-1.0, 1.0);
        }

        if let Some(ref mut wrapper) = self.resampler {
            wrapper.input_buffer.extend_from_slice(&samples);
            let needed_frames = wrapper.chunk_size;
            let needed_samples = needed_frames * wrapper.channels;

            let mut resampled_out = Vec::new();

            while wrapper.input_buffer.len() >= needed_samples {
                let chunk: Vec<f32> = wrapper.input_buffer.drain(..needed_samples).collect();
                let input_owned = InterleavedOwned::<f32>::new_from(chunk, wrapper.channels, needed_frames)
                    .map_err(|e| AudioError::Resample(e.to_string()))?;

                let output = wrapper
                    .resampler
                    .process(&input_owned, None)
                    .map_err(|e| AudioError::Resample(e.to_string()))?;

                resampled_out.extend(output.take_data());
            }

            Ok(resampled_out)
        } else {
            Ok(samples)
        }
    }
}

pub enum TargetSampleFormat {
    Pcm16,
    Pcm24In32,
    Pcm24Packed,
    Pcm32,
    Float32,
}

pub fn convert_f32_to_bytes(samples: &[f32], format: TargetSampleFormat) -> Vec<u8> {
    match format {
        TargetSampleFormat::Pcm16 => {
            let mut bytes = Vec::with_capacity(samples.len() * 2);
            for &s in samples {
                let val = (s.clamp(-1.0, 1.0) * 32767.0) as i16;
                bytes.extend_from_slice(&val.to_le_bytes());
            }
            bytes
        }
        TargetSampleFormat::Pcm24Packed => {
            let mut bytes = Vec::with_capacity(samples.len() * 3);
            for &s in samples {
                let val = (s.clamp(-1.0, 1.0) * 8388607.0) as i32;
                let le = val.to_le_bytes();
                bytes.push(le[0]);
                bytes.push(le[1]);
                bytes.push(le[2]);
            }
            bytes
        }
        TargetSampleFormat::Pcm24In32 => {
            let mut bytes = Vec::with_capacity(samples.len() * 4);
            for &s in samples {
                let val = (s.clamp(-1.0, 1.0) * 8388607.0) as i32;
                let padded = val << 8;
                bytes.extend_from_slice(&padded.to_le_bytes());
            }
            bytes
        }
        TargetSampleFormat::Pcm32 => {
            let mut bytes = Vec::with_capacity(samples.len() * 4);
            for &s in samples {
                let val = (s.clamp(-1.0, 1.0) * 2147483647.0) as i32;
                bytes.extend_from_slice(&val.to_le_bytes());
            }
            bytes
        }
        TargetSampleFormat::Float32 => {
            let mut bytes = Vec::with_capacity(samples.len() * 4);
            for &s in samples {
                bytes.extend_from_slice(&s.to_le_bytes());
            }
            bytes
        }
    }
}

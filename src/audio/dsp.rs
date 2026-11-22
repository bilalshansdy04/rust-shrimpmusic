use std::f32::consts::PI;
use rubato::audioadapter_buffers::owned::InterleavedOwned;
use rubato::{Fft, FixedSync, Resampler};
use super::error::{AudioError, AudioResult};

pub const EQ_FREQUENCIES: [f32; 10] = [
    31.25, 62.5, 125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0, 8000.0, 16000.0,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EqPreset {
    Flat,
    BassBoost,
    VocalClear,
    Acoustic,
    TrebleBoost,
    Custom,
}

impl EqPreset {
    pub fn gains(&self) -> [f32; 10] {
        match self {
            Self::Flat => [0.0; 10],
            Self::BassBoost => [4.0, 5.0, 3.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            Self::VocalClear => [0.0, -1.0, -1.0, 0.0, 1.0, 3.0, 4.0, 2.0, 0.0, 0.0],
            Self::Acoustic => [1.0, 2.0, 2.0, 3.0, 1.0, 1.0, 2.0, 3.0, 1.0, 0.0],
            Self::TrebleBoost => [0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 2.0, 4.0, 6.0],
            Self::Custom => [0.0; 10],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VolumeBalanceMode {
    Off,
    Quiet,    // -18 LUFS (gentle / safe)
    Balanced, // -14 LUFS (standard)
    Punchy,   // -11 LUFS (loud / punchy)
}

impl Default for VolumeBalanceMode {
    fn default() -> Self {
        Self::Off
    }
}

impl VolumeBalanceMode {
    pub fn from_i32(val: i32) -> Self {
        match val {
            1 => Self::Quiet,
            2 => Self::Balanced,
            3 => Self::Punchy,
            _ => Self::Off,
        }
    }

    pub fn to_i32(self) -> i32 {
        match self {
            Self::Off => 0,
            Self::Quiet => 1,
            Self::Balanced => 2,
            Self::Punchy => 3,
        }
    }

    pub fn gain_multiplier(&self) -> f32 {
        match self {
            Self::Off => 1.0,
            Self::Quiet => 0.72,
            Self::Balanced => 1.0,
            Self::Punchy => 1.35,
        }
    }
}

#[derive(Debug, Clone)]
struct BiquadCoeffs {
    b0: f32,
    b1: f32,
    b2: f32,
    a1: f32,
    a2: f32,
}

impl Default for BiquadCoeffs {
    fn default() -> Self {
        Self {
            b0: 1.0,
            b1: 0.0,
            b2: 0.0,
            a1: 0.0,
            a2: 0.0,
        }
    }
}

#[derive(Debug, Clone, Default)]
struct BiquadChannelState {
    x1: f32,
    x2: f32,
    y1: f32,
    y2: f32,
}

impl BiquadChannelState {
    #[inline(always)]
    fn process(&mut self, input: f32, coeffs: &BiquadCoeffs) -> f32 {
        let output = coeffs.b0 * input + coeffs.b1 * self.x1 + coeffs.b2 * self.x2
            - coeffs.a1 * self.y1
            - coeffs.a2 * self.y2;

        self.x2 = self.x1;
        self.x1 = input;
        self.y2 = self.y1;
        self.y1 = output;

        output
    }

    fn reset(&mut self) {
        self.x1 = 0.0;
        self.x2 = 0.0;
        self.y1 = 0.0;
        self.y2 = 0.0;
    }
}

#[derive(Debug, Clone)]
pub struct Equalizer10Band {
    enabled: bool,
    gains_db: [f32; 10],
    sample_rate: u32,
    channels: usize,
    coeffs: [BiquadCoeffs; 10],
    states: Vec<[BiquadChannelState; 10]>,
}

impl Equalizer10Band {
    pub fn new(sample_rate: u32, channels: usize) -> Self {
        let mut eq = Self {
            enabled: false,
            gains_db: [0.0; 10],
            sample_rate: sample_rate.max(44100),
            channels: channels.max(1),
            coeffs: std::array::from_fn(|_| BiquadCoeffs::default()),
            states: vec![std::array::from_fn(|_| BiquadChannelState::default()); channels.max(1)],
        };
        eq.recalculate_coeffs();
        eq
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        if !enabled {
            self.reset_states();
        }
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_band_gain(&mut self, band: usize, gain_db: f32) {
        if band < 10 {
            self.gains_db[band] = gain_db.clamp(-12.0, 12.0);
            self.recalculate_band(band);
        }
    }

    pub fn apply_preset(&mut self, preset: EqPreset) {
        self.gains_db = preset.gains();
        self.recalculate_coeffs();
    }

    pub fn gains(&self) -> [f32; 10] {
        self.gains_db
    }

    pub fn update_sample_rate(&mut self, sample_rate: u32, channels: usize) {
        self.sample_rate = sample_rate.max(44100);
        self.channels = channels.max(1);
        self.states = vec![std::array::from_fn(|_| BiquadChannelState::default()); self.channels];
        self.recalculate_coeffs();
    }

    pub fn reset_states(&mut self) {
        for ch in &mut self.states {
            for band_state in ch.iter_mut() {
                band_state.reset();
            }
        }
    }

    fn recalculate_band(&mut self, band: usize) {
        let f0 = EQ_FREQUENCIES[band];
        let fs = self.sample_rate as f32;
        let gain_db = self.gains_db[band];

        if f0 >= fs / 2.0 {
            self.coeffs[band] = BiquadCoeffs::default();
            return;
        }

        let a = 10.0_f32.powf(gain_db / 40.0);
        let w0 = 2.0 * PI * f0 / fs;
        let q = 1.414_f32; // ~1 octave bandwidth
        let alpha = w0.sin() / (2.0 * q);
        let cos_w0 = w0.cos();

        let a0 = 1.0 + alpha / a;
        let b0 = (1.0 + alpha * a) / a0;
        let b1 = (-2.0 * cos_w0) / a0;
        let b2 = (1.0 - alpha * a) / a0;
        let a1 = (-2.0 * cos_w0) / a0;
        let a2 = (1.0 - alpha / a) / a0;

        self.coeffs[band] = BiquadCoeffs { b0, b1, b2, a1, a2 };
    }

    fn recalculate_coeffs(&mut self) {
        for band in 0..10 {
            self.recalculate_band(band);
        }
    }

    pub fn process_interleaved(&mut self, samples: &mut [f32]) {
        if !self.enabled || self.channels == 0 {
            return; // 0% overhead when bypassed
        }

        for (idx, sample) in samples.iter_mut().enumerate() {
            let ch = idx % self.channels;
            if ch < self.states.len() {
                let mut current = *sample;
                for band in 0..10 {
                    current = self.states[ch][band].process(current, &self.coeffs[band]);
                }
                *sample = current;
            }
        }
    }
}

pub struct DspProcessor {
    volume: f32,
    volume_balance: VolumeBalanceMode,
    equalizer: Equalizer10Band,
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
            volume_balance: VolumeBalanceMode::Off,
            equalizer: Equalizer10Band::new(44100, 2),
            resampler: None,
        }
    }

    pub fn set_volume(&mut self, volume: f32) {
        self.volume = volume.clamp(0.0, 1.0);
    }

    pub fn volume(&self) -> f32 {
        self.volume
    }

    pub fn set_volume_balance(&mut self, mode: VolumeBalanceMode) {
        self.volume_balance = mode;
    }

    pub fn volume_balance(&self) -> VolumeBalanceMode {
        self.volume_balance
    }

    pub fn equalizer_mut(&mut self) -> &mut Equalizer10Band {
        &mut self.equalizer
    }

    pub fn equalizer(&self) -> &Equalizer10Band {
        &self.equalizer
    }

    pub fn setup_resampler(
        &mut self,
        source_rate: u32,
        target_rate: u32,
        channels: usize,
    ) -> AudioResult<()> {
        self.equalizer.update_sample_rate(source_rate, channels);

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
        // 1. Equalizer (Bypassed if disabled)
        self.equalizer.process_interleaved(&mut samples);

        // 2. Volume & ReplayGain Normalization
        let vol_gain = self.volume * self.volume;
        let balance_gain = self.volume_balance.gain_multiplier();
        let total_gain = vol_gain * balance_gain;

        for s in samples.iter_mut() {
            let amplified = *s * total_gain;
            *s = soft_clip(amplified);
        }

        // 3. Resampler (if hardware sample rate does not match source)
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

#[inline(always)]
fn soft_clip(sample: f32) -> f32 {
    if sample > 0.95 {
        0.95 + 0.05 * ((sample - 0.95) / 0.05).tanh()
    } else if sample < -0.95 {
        -0.95 + 0.05 * ((sample + 0.95) / 0.05).tanh()
    } else {
        sample
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

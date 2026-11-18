use std::f32::consts::PI;

pub struct SafetyTestSignal;

impl SafetyTestSignal {
    /// Generates 10 seconds of calibrated -18 dBFS gentle harmonic chime audio
    /// at the given sample rate and channel count.
    pub fn generate_10s_chime(sample_rate: u32, channels: u16) -> Vec<f32> {
        let total_frames = (sample_rate as f64 * 10.0) as usize;
        let mut samples = Vec::with_capacity(total_frames * channels as usize);

        // -18 dBFS amplitude
        let peak_amp = 10.0_f32.powf(-18.0 / 20.0); // ~0.1259

        // Pulse chime every 2.0 seconds (5 pulses across 10s)
        let pulse_duration = 2.0_f32;

        for frame in 0..total_frames {
            let t = frame as f32 / sample_rate as f32;
            let pulse_t = t % pulse_duration;

            // Exponential decay envelope for the chime pulse
            let envelope = (-3.5 * pulse_t).exp();

            // Multi-harmonic chime (A4 = 440Hz, E5 = 660Hz, A5 = 880Hz)
            let fundamental = (2.0 * PI * 440.0 * pulse_t).sin();
            let second_harmonic = 0.5 * (2.0 * PI * 880.0 * pulse_t).sin();
            let third_harmonic = 0.25 * (2.0 * PI * 1320.0 * pulse_t).sin();

            let raw = (fundamental + second_harmonic + third_harmonic) / 1.75;
            let sample = raw * envelope * peak_amp;

            for _ in 0..channels {
                samples.push(sample);
            }
        }

        samples
    }
}

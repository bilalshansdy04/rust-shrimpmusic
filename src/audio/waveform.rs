use std::path::Path;
use super::decoder::SymphoniaDecoder;

pub const WAVEFORM_BAR_COUNT: usize = 120;

pub fn generate_waveform_peaks(path: &Path) -> Result<Vec<f32>, Box<dyn std::error::Error + Send + Sync>> {
    let mut decoder = match SymphoniaDecoder::open(path) {
        Ok(d) => d,
        Err(_) => return Ok(generate_synthetic_waveform()),
    };

    let mut raw_peaks: Vec<f32> = Vec::new();

    while let Ok(Some(samples)) = decoder.next_packet_samples() {
        let peak = samples.iter().cloned().fold(0.0_f32, |max, s| max.max(s.abs()));
        raw_peaks.push(peak);
    }

    if raw_peaks.is_empty() {
        return Ok(generate_synthetic_waveform());
    }

    let bucket_size = (raw_peaks.len() as f64 / WAVEFORM_BAR_COUNT as f64).max(1.0);
    let mut bars = Vec::with_capacity(WAVEFORM_BAR_COUNT);

    for i in 0..WAVEFORM_BAR_COUNT {
        let start_idx = (i as f64 * bucket_size) as usize;
        let end_idx = (((i + 1) as f64 * bucket_size) as usize).min(raw_peaks.len());

        let peak = if start_idx < raw_peaks.len() {
            raw_peaks[start_idx..end_idx]
                .iter()
                .cloned()
                .fold(0.0_f32, |max, val| max.max(val))
        } else {
            0.1
        };
        bars.push(peak);
    }

    let max_val = bars.iter().cloned().fold(0.0_f32, |m, v| m.max(v));
    let norm_factor = if max_val > 0.001 { 1.0 / max_val } else { 1.0 };

    for b in bars.iter_mut() {
        *b = (*b * norm_factor).clamp(0.08, 1.0);
    }

    Ok(bars)
}

pub fn generate_synthetic_waveform() -> Vec<f32> {
    let mut bars = Vec::with_capacity(WAVEFORM_BAR_COUNT);
    for i in 0..WAVEFORM_BAR_COUNT {
        let t = i as f32 / WAVEFORM_BAR_COUNT as f32;
        let v = 0.2 + 0.6 * (t * std::f32::consts::PI).sin() + 0.15 * (t * 8.0).sin().abs();
        bars.push(v.clamp(0.08, 1.0));
    }
    bars
}

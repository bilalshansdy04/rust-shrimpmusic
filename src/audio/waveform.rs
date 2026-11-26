use std::path::Path;
use super::decoder::SymphoniaDecoder;

pub const WAVEFORM_BAR_COUNT: usize = 120;

pub fn generate_waveform_peaks(path: &Path) -> Result<Vec<f32>, Box<dyn std::error::Error + Send + Sync>> {
    let mut decoder = match SymphoniaDecoder::open(path) {
        Ok(d) => d,
        Err(_) => return Ok(generate_synthetic_waveform()),
    };

    let mut raw_rms: Vec<f32> = Vec::new();

    while let Ok(Some(samples)) = decoder.next_packet_samples() {
        if samples.is_empty() {
            continue;
        }
        let sum_sq: f64 = samples.iter().map(|&s| (s as f64) * (s as f64)).sum();
        let rms = (sum_sq / samples.len() as f64).sqrt() as f32;
        raw_rms.push(rms);
    }

    if raw_rms.is_empty() {
        return Ok(generate_synthetic_waveform());
    }

    let bucket_size = (raw_rms.len() as f64 / WAVEFORM_BAR_COUNT as f64).max(1.0);
    let mut bars = Vec::with_capacity(WAVEFORM_BAR_COUNT);

    for i in 0..WAVEFORM_BAR_COUNT {
        let start_idx = (i as f64 * bucket_size) as usize;
        let end_idx = (((i + 1) as f64 * bucket_size) as usize).min(raw_rms.len());

        let rms = if start_idx < end_idx {
            let slice = &raw_rms[start_idx..end_idx];
            let sum_sq: f64 = slice.iter().map(|&v| (v as f64) * (v as f64)).sum();
            (sum_sq / slice.len() as f64).sqrt() as f32
        } else {
            0.08
        };
        bars.push(rms);
    }

    let max_val = bars.iter().cloned().fold(0.0_f32, |m, v| m.max(v));
    if max_val <= 0.0001 {
        return Ok(vec![0.08; WAVEFORM_BAR_COUNT]);
    }

    // Dynamic range expansion so loudness-maximized tracks still show clear verse/chorus contrast
    let min_val = bars.iter().cloned().fold(f32::MAX, |m, v| m.min(v));
    let floor = min_val * 0.5;
    let range = (max_val - floor).max(0.0001);

    for b in bars.iter_mut() {
        let normalized = ((*b - floor) / range).clamp(0.0, 1.0);
        let curved = normalized.powf(1.45);
        *b = (0.08 + 0.92 * curved).clamp(0.08, 1.0);
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

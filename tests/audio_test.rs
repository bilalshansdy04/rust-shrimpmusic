use shrimp_music::audio::dsp::{convert_f32_to_bytes, DspProcessor, TargetSampleFormat};
use shrimp_music::audio::device::DeviceManager;

#[test]
fn test_dsp_volume_curve() {
    let mut dsp = DspProcessor::new();
    assert_eq!(dsp.volume(), 1.0);

    // PRD: Volume curve Gain = Volume^2
    dsp.set_volume(0.5);
    assert_eq!(dsp.volume(), 0.5);

    let input = vec![1.0, -1.0, 0.0];
    let output = dsp.process_samples(input).unwrap();
    // 0.5^2 = 0.25 gain
    assert!((output[0] - 0.25).abs() < 1e-5);
    assert!((output[1] - (-0.25)).abs() < 1e-5);
    assert!((output[2] - 0.0).abs() < 1e-5);
}

#[test]
fn test_dsp_clipping_protection() {
    let mut dsp = DspProcessor::new();
    dsp.set_volume(1.0);

    let input = vec![1.5, -2.0];
    let output = dsp.process_samples(input).unwrap();
    assert_eq!(output[0], 1.0);
    assert_eq!(output[1], -1.0);
}

#[test]
fn test_pcm16_conversion() {
    let samples = vec![0.0, 1.0, -1.0];
    let bytes = convert_f32_to_bytes(&samples, TargetSampleFormat::Pcm16);
    // 3 samples * 2 bytes = 6 bytes
    assert_eq!(bytes.len(), 6);

    let s0 = i16::from_le_bytes([bytes[0], bytes[1]]);
    let s1 = i16::from_le_bytes([bytes[2], bytes[3]]);
    let s2 = i16::from_le_bytes([bytes[4], bytes[5]]);

    assert_eq!(s0, 0);
    assert_eq!(s1, 32767);
    assert_eq!(s2, -32767);
}

#[test]
fn test_pcm24_packed_conversion() {
    let samples = vec![1.0];
    let bytes = convert_f32_to_bytes(&samples, TargetSampleFormat::Pcm24Packed);
    assert_eq!(bytes.len(), 3);
    assert_eq!(bytes, vec![0xFF, 0xFF, 0x7F]); // 8388607 in 24-bit LE
}

#[test]
fn test_device_enumeration() {
    let dm = DeviceManager::new().expect("Failed to initialize DeviceManager");
    let devices = dm.list_output_devices().expect("Failed to list output devices");
    assert!(!devices.is_empty(), "Should find at least one audio output device on Windows");
    let default_count = devices.iter().filter(|d| d.is_default).count();
    assert_eq!(default_count, 1, "Should have exactly one default output device");
}

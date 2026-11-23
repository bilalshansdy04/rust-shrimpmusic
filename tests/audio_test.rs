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

#[test]
fn test_device_classification_and_bit_perfect_support() {
    use shrimp_music::audio::device::{AudioDeviceInfo, DeviceClassification};

    // Test Type-C DAC converter with chip
    let (c1, sup1) = AudioDeviceInfo::classify("Headphones (JadeAudio JA11)", r"USB\VID_2972&PID_0055\000000000001");
    assert_eq!(c1, DeviceClassification::TypeCDacConverter);
    assert!(sup1);

    // Test CX31993 Type-C DAC dongle
    let (c2, sup2) = AudioDeviceInfo::classify("Conexant CX31993 HiFi Audio", r"USB\VID_1F3A&PID_1001");
    assert_eq!(c2, DeviceClassification::TypeCDacConverter);
    assert!(sup2);

    // Test Dedicated USB DAC
    let (c3, sup3) = AudioDeviceInfo::classify("FiiO Q3 USB DAC", r"USB\VID_2972&PID_0044");
    assert_eq!(c3, DeviceClassification::UsbDac);
    assert!(sup3);

    // Test Onboard Realtek Speakers
    let (c4, sup4) = AudioDeviceInfo::classify("Speakers (Realtek(R) Audio)", r"INTELAUDIO\FUNC_01&VEN_10EC");
    assert_eq!(c4, DeviceClassification::InternalSpeakers);
    assert!(!sup4, "Internal speakers should not support bit-perfect exclusive mode");

    // Test Virtual audio enhancer
    let (c5, sup5) = AudioDeviceInfo::classify("FxSound Speakers (FxSound Audio Enhancer)", r"ROOT\MEDIA\0000");
    assert_eq!(c5, DeviceClassification::VirtualAudio);
    assert!(!sup5, "Virtual audio should not support bit-perfect exclusive mode");
}

#[test]
fn test_decoder_seek() {
    let flac_path = std::path::Path::new(r"D:\Lossless Music\01 - Love Me Back.flac");
    if flac_path.exists() {
        let mut dec = shrimp_music::audio::decoder::SymphoniaDecoder::open(flac_path).expect("Failed to open FLAC");
        let seek_res = dec.seek(std::time::Duration::from_secs(15));
        println!("Seek result: {:?}", seek_res);
        assert!(seek_res.is_ok(), "Seek should succeed: {:?}", seek_res);
    }
}

#[test]
fn test_windows_master_volume() {
    let ctrl = shrimp_music::audio::WindowsVolumeController::new();
    std::thread::sleep(std::time::Duration::from_millis(150));
    let vol = ctrl.get_volume();
    println!("Windows master volume from controller: {}", vol);
    assert!(vol >= 0.0 && vol <= 1.0);
}

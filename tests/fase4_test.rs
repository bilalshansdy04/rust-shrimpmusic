use shrimp_music::audio::dsp::{DspProcessor, EqPreset, VolumeBalanceMode};
use shrimp_music::db::{open_memory_database, TrackRecord};
use shrimp_music::ui::app::AppWindow;
use shrimp_music::ui::taskbar::TaskbarController;
use slint::{Model, ModelRc, VecModel};
use std::cell::Cell;
use std::rc::Rc;

#[test]
fn test_fase4_dsp_eq_bypass_and_processing() {
    let mut dsp = DspProcessor::new();

    // 1. Bit-Perfect Bypass Test: when disabled, samples must be 100% untouched
    dsp.equalizer_mut().set_enabled(false);
    assert!(!dsp.equalizer().is_enabled());

    let original_samples = vec![0.1f32, -0.2, 0.5, -0.8, 0.3, -0.1];
    let mut bypass_buffer = original_samples.clone();
    dsp.equalizer_mut().process_interleaved(&mut bypass_buffer);
    assert_eq!(
        original_samples, bypass_buffer,
        "Bit-Perfect bypass must leave samples completely identical"
    );

    // 2. Active EQ Processing Test
    dsp.equalizer_mut().set_enabled(true);
    assert!(dsp.equalizer().is_enabled());

    // Set BassBoost (+6dB on low bands)
    dsp.equalizer_mut().apply_preset(EqPreset::BassBoost);
    let mut filtered_buffer = original_samples.clone();
    dsp.equalizer_mut().process_interleaved(&mut filtered_buffer);

    // The processed samples should differ from original due to filtering
    assert_ne!(
        original_samples, filtered_buffer,
        "Active EQ with BassBoost must filter audio samples"
    );

    // 3. Preset Gains Verification
    let flat_gains = EqPreset::Flat.gains();
    assert!(flat_gains.iter().all(|&g| g == 0.0));

    let bass_gains = EqPreset::BassBoost.gains();
    assert_eq!(bass_gains[0], 4.0); // 31.25 Hz
    assert_eq!(bass_gains[1], 5.0); // 62.5 Hz
    assert_eq!(bass_gains[9], 0.0); // 16 kHz

    let vocal_gains = EqPreset::VocalClear.gains();
    assert_eq!(vocal_gains[4], 1.0); // 500 Hz
    assert_eq!(vocal_gains[5], 3.0); // 1 kHz
}

#[test]
fn test_fase4_replay_gain_and_soft_limiter() {
    let mut dsp = DspProcessor::new();

    // 1. Volume Balance Mode Gains
    dsp.set_volume_balance(VolumeBalanceMode::Off);
    assert_eq!(dsp.volume_balance(), VolumeBalanceMode::Off);

    dsp.set_volume_balance(VolumeBalanceMode::Quiet);
    assert_eq!(dsp.volume_balance(), VolumeBalanceMode::Quiet);

    dsp.set_volume_balance(VolumeBalanceMode::Balanced);
    assert_eq!(dsp.volume_balance(), VolumeBalanceMode::Balanced);

    dsp.set_volume_balance(VolumeBalanceMode::Punchy);
    assert_eq!(dsp.volume_balance(), VolumeBalanceMode::Punchy);

    // 2. Soft-Limiter Anti-Clipping Verification via process_samples
    dsp.set_volume(1.0);
    dsp.set_volume_balance(VolumeBalanceMode::Punchy); // +2.0 dB boost

    // Extremely loud sample that would clip (> 1.0)
    let hot_buffer = vec![1.8f32, -2.5f32, 1.2f32, -1.5f32];
    let processed = dsp.process_samples(hot_buffer).expect("DSP processing failed");

    for &sample in &processed {
        assert!(
            sample >= -1.0 && sample <= 1.0,
            "Soft-clipping limiter must ensure samples strictly remain within [-1.0, 1.0], got {}",
            sample
        );
    }
}

#[test]
fn test_fase4_waveform_database_cache() {
    let db = open_memory_database().expect("Failed to open test in-memory database");

    // Create a mock folder & track
    let folder_id = db
        .add_folder("C:\\Music\\HighRes")
        .expect("Failed to add folder");

    let track = TrackRecord {
        id: 0,
        folder_id,
        path: "C:\\Music\\HighRes\\track01.flac".to_string(),
        title: "Audiophile Showcase".to_string(),
        artist: "Master Sound".to_string(),
        album: "Direct Stream Digital".to_string(),
        album_artist: None,
        genre: Some("Audiophile".to_string()),
        year: Some(2024),
        track_number: Some(1),
        disc_number: Some(1),
        duration_ms: 180000,
        sample_rate: 192000,
        bit_depth: 24,
        format: "FLAC".to_string(),
        file_size: 45000000,
        modified_time: 1700000000,
        thumbnail_path: None,
        has_lyrics: false,
        play_count: 0,
        last_played_at: None,
        waveform: None,
        is_active: true,
    };

    let track_id = db.upsert_track(&track).expect("Failed to upsert track");

    // Initially waveform is None
    let cached = db
        .get_track_waveform(track_id)
        .expect("Failed to query waveform");
    assert!(cached.is_none());

    // Generate mock 120-bar waveform data
    let mut mock_peaks = Vec::with_capacity(120);
    for i in 0..120 {
        mock_peaks.push((i as f32 / 120.0).sin().abs());
    }
    assert_eq!(mock_peaks.len(), 120);

    // Save to DB
    db.update_track_waveform(track_id, &mock_peaks)
        .expect("Failed to update waveform");

    // Retrieve from DB
    let retrieved = db
        .get_track_waveform(track_id)
        .expect("Failed to get waveform")
        .expect("Waveform should exist in database");

    assert_eq!(retrieved.len(), 120);
    for (orig, ret) in mock_peaks.iter().zip(retrieved.iter()) {
        assert!((orig - ret).abs() < 1e-6);
    }
}

#[test]
fn test_fase4_taskbar_controller() {
    let mut tb = TaskbarController::new();
    // Test that safe calling without attached HWND does not crash
    tb.set_playback_state(true);
    tb.set_playback_state(false);
}

#[test]
fn test_fase4_slint_ui_pipeline() {
    let window = AppWindow::new().expect("Failed to create AppWindow");

    // 1. Initial EQ states
    assert_eq!(window.get_is_eq_enabled(), false);
    assert_eq!(window.get_volume_balance_mode(), 0);
    assert_eq!(window.get_is_audiophile_modal_visible(), false);

    // 2. Set EQ & Presets on Window
    window.set_is_eq_enabled(true);
    assert_eq!(window.get_is_eq_enabled(), true);

    let gains = EqPreset::BassBoost.gains();
    window.set_eq_bands(ModelRc::new(VecModel::from(gains.to_vec())));
    let model = window.get_eq_bands();
    assert_eq!(model.row_count(), 10);
    assert_eq!(model.row_data(0), Some(4.0));

    // 3. Audiophile Recommendation Dialog
    window.set_is_audiophile_modal_visible(true);
    assert!(window.get_is_audiophile_modal_visible());

    let bypass_confirmed = Rc::new(Cell::new(false));
    let bc_clone = bypass_confirmed.clone();
    window.on_on_confirm_bypass_eq(move || {
        bc_clone.set(true);
    });

    window.invoke_on_confirm_bypass_eq();
    assert!(bypass_confirmed.get());

    // 4. Waveform 120 Peaks UI Property
    let mut peaks = Vec::new();
    for i in 0..120 {
        peaks.push(i as f32 / 120.0);
    }
    window.set_waveform_peaks(ModelRc::new(VecModel::from(peaks)));
    assert_eq!(window.get_waveform_peaks().row_count(), 120);

    // 5. Kinetic Lyrics Scroll properties
    assert_eq!(window.get_is_user_scrolling_lyrics(), false);
    window.set_is_user_scrolling_lyrics(true);
    assert!(window.get_is_user_scrolling_lyrics());

    window.set_lyrics_scroll_offset_y(-150.0);
    assert_eq!(window.get_lyrics_scroll_offset_y(), -150.0);

    // 6. Rescan Folder Callback
    let rescan_id = Rc::new(Cell::new(-1));
    let r_clone = rescan_id.clone();
    window.on_on_rescan_folder(move |id| {
        r_clone.set(id);
    });

    window.invoke_on_rescan_folder(42);
    assert_eq!(rescan_id.get(), 42);

    // 7. Add Folder Clicked Callback
    let add_folder_invoked = Rc::new(Cell::new(false));
    let af_clone = add_folder_invoked.clone();
    window.on_on_add_folder_clicked(move || {
        af_clone.set(true);
    });

    window.invoke_on_add_folder_clicked();
    assert!(add_folder_invoked.get());
}

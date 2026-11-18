use shrimp_music::audio::{LoopMode, PlaybackQueue, SafetyTestSignal, ShuffleStyle};
use shrimp_music::db::{open_memory_database, SortCriteria, TrackRecord};
use shrimp_music::library::artwork::{clear_thumbnail_cache, generate_and_save_thumbnail};
use shrimp_music::library::metadata::{check_is_synced, parse_lrc_lines};
use shrimp_music::library::playlist::PlaylistManager;
use shrimp_music::library::search::SearchEngine;

#[test]
fn test_database_and_scoped_library() {
    let db = open_memory_database().expect("Failed to open in-memory db");

    let f1_id = db.add_folder("D:\\Lossless").expect("Failed to add folder 1");
    let f2_id = db.add_folder("E:\\MyFavMusic").expect("Failed to add folder 2");

    let tr1 = TrackRecord {
        id: 0,
        folder_id: f1_id,
        path: "D:\\Lossless\\song_b.flac".to_string(),
        title: "Beta Song".to_string(),
        artist: "Artist One".to_string(),
        album: "Album A".to_string(),
        album_artist: None,
        genre: Some("Rock".to_string()),
        year: Some(2023),
        track_number: Some(2),
        disc_number: Some(1),
        duration_ms: 180_000,
        sample_rate: 96000,
        bit_depth: 24,
        format: "FLAC".to_string(),
        file_size: 20_000_000,
        modified_time: 1000,
        thumbnail_path: None,
        has_lyrics: false,
        play_count: 5,
        last_played_at: None,
        is_active: true,
    };

    let tr2 = TrackRecord {
        id: 0,
        folder_id: f1_id,
        path: "D:\\Lossless\\song_a.flac".to_string(),
        title: "Alpha Song".to_string(),
        artist: "Artist Two".to_string(),
        album: "Album B".to_string(),
        album_artist: None,
        genre: Some("Jazz".to_string()),
        year: Some(2021),
        track_number: Some(1),
        disc_number: Some(1),
        duration_ms: 240_000,
        sample_rate: 192000,
        bit_depth: 24,
        format: "FLAC".to_string(),
        file_size: 45_000_000,
        modified_time: 2000,
        thumbnail_path: None,
        has_lyrics: true,
        play_count: 10,
        last_played_at: None,
        is_active: true,
    };

    let tr3 = TrackRecord {
        id: 0,
        folder_id: f2_id,
        path: "E:\\MyFavMusic\\song_c.m4a".to_string(),
        title: "Gamma Song".to_string(),
        artist: "Artist One".to_string(),
        album: "Album C".to_string(),
        album_artist: None,
        genre: Some("Pop".to_string()),
        year: Some(2024),
        track_number: Some(1),
        disc_number: Some(1),
        duration_ms: 210_000,
        sample_rate: 44100,
        bit_depth: 16,
        format: "M4A".to_string(),
        file_size: 8_000_000,
        modified_time: 3000,
        thumbnail_path: None,
        has_lyrics: false,
        play_count: 2,
        last_played_at: None,
        is_active: true,
    };

    let t1_id = db.upsert_track(&tr1).unwrap();
    let _t2_id = db.upsert_track(&tr2).unwrap();
    let _t3_id = db.upsert_track(&tr3).unwrap();

    // 1. All Folders scope (Default: Title A-Z)
    let all_tracks = db.list_tracks(None, SortCriteria::Title, true).unwrap();
    assert_eq!(all_tracks.len(), 3);
    assert_eq!(all_tracks[0].title, "Alpha Song");
    assert_eq!(all_tracks[1].title, "Beta Song");
    assert_eq!(all_tracks[2].title, "Gamma Song");

    // 2. Sorting by Audio Specs (highest sample rate first)
    let hi_res_first = db.list_tracks(None, SortCriteria::AudioSpecs, false).unwrap();
    assert_eq!(hi_res_first[0].title, "Alpha Song"); // 192kHz
    assert_eq!(hi_res_first[1].title, "Beta Song");  // 96kHz
    assert_eq!(hi_res_first[2].title, "Gamma Song"); // 44.1kHz

    // 3. Per-folder scope: only folder 2
    let f2_tracks = db.list_tracks(Some(f2_id), SortCriteria::Title, true).unwrap();
    assert_eq!(f2_tracks.len(), 1);
    assert_eq!(f2_tracks[0].title, "Gamma Song");

    // 4. Folder toggle switch test: disable Folder 1
    db.set_folder_enabled(f1_id, false).unwrap();
    let active_all = db.list_tracks(None, SortCriteria::Title, true).unwrap();
    assert_eq!(active_all.len(), 1);
    assert_eq!(active_all[0].title, "Gamma Song");

    // Re-enable Folder 1
    db.set_folder_enabled(f1_id, true).unwrap();
    let re_enabled = db.list_tracks(None, SortCriteria::Title, true).unwrap();
    assert_eq!(re_enabled.len(), 3);

    // 5. User Preferences persistence
    db.set_preference("volume", "0.75").unwrap();
    assert_eq!(db.get_preference("volume").unwrap().as_deref(), Some("0.75"));
    db.set_preference("sort_criteria", "0").unwrap();
    assert_eq!(db.get_preference("sort_criteria").unwrap().as_deref(), Some("0"));

    // 6. Play count update
    db.increment_play_count(t1_id, 12345).unwrap();
    let updated_t1 = db.get_track_by_id(t1_id).unwrap().unwrap();
    assert_eq!(updated_t1.play_count, 6);
    assert_eq!(updated_t1.last_played_at, Some(12345));
}

#[test]
fn test_lyrics_parsing_and_search() {
    let raw_lrc = r#"
[00:05.00]First line of lyrics
[00:12.50]Second line with emotional melody
[00:20.00][00:30.00]Dual timestamp chorus line
"#;

    assert!(check_is_synced(raw_lrc));
    let parsed = parse_lrc_lines(raw_lrc);
    assert_eq!(parsed.len(), 4);
    assert_eq!(parsed[0].timestamp_ms, 5_000);
    assert_eq!(parsed[0].text, "First line of lyrics");
    assert_eq!(parsed[1].timestamp_ms, 12_500);
    assert_eq!(parsed[2].timestamp_ms, 20_000);
    assert_eq!(parsed[3].timestamp_ms, 30_000);
    assert_eq!(parsed[3].text, "Dual timestamp chorus line");

    // Test DB search with lyrics
    let db = open_memory_database().expect("Failed to open db");
    let fid = db.add_folder("D:\\Music").unwrap();

    let tr = TrackRecord {
        id: 0,
        folder_id: fid,
        path: "D:\\Music\\ballad.flac".to_string(),
        title: "Autumn Leaves".to_string(),
        artist: "Eva".to_string(),
        album: "Classic Memories".to_string(),
        album_artist: None,
        genre: None,
        year: None,
        track_number: None,
        disc_number: None,
        duration_ms: 180_000,
        sample_rate: 44100,
        bit_depth: 16,
        format: "FLAC".to_string(),
        file_size: 10_000_000,
        modified_time: 1000,
        thumbnail_path: None,
        has_lyrics: true,
        play_count: 0,
        last_played_at: None,
        is_active: true,
    };
    let tid = db.upsert_track(&tr).unwrap();
    db.set_lyrics(tid, true, raw_lrc).unwrap();

    // 1. General search: finds both track and lyric
    let res = SearchEngine::search(&db, "emotional").unwrap();
    assert_eq!(res.lyrics.len(), 1);
    assert_eq!(res.lyrics[0].track_id, tid);
    assert_eq!(res.lyrics[0].timestamp_ms, 12_500);
    assert!(res.lyrics[0].snippet.contains("emotional"));

    // 2. Explicit prefix search: `lirik:chorus`
    let lyric_res = SearchEngine::search(&db, "lirik:chorus").unwrap();
    assert_eq!(lyric_res.tracks.len(), 0);
    assert_eq!(lyric_res.lyrics.len(), 1);

    // 3. Artist search: `artist:Eva`
    let artist_res = SearchEngine::search(&db, "artist:Eva").unwrap();
    assert_eq!(artist_res.tracks.len(), 1);
    assert_eq!(artist_res.tracks[0].title, "Autumn Leaves");
}

#[test]
fn test_playlist_and_m3u8_import_export() {
    let db = open_memory_database().expect("Failed to open db");
    let fid = db.add_folder("D:\\Lossless").unwrap();

    let tr1 = TrackRecord {
        id: 0,
        folder_id: fid,
        path: "D:\\Lossless\\song1.flac".to_string(),
        title: "Song One".to_string(),
        artist: "Band A".to_string(),
        album: "First Album".to_string(),
        album_artist: None,
        genre: None,
        year: None,
        track_number: Some(1),
        disc_number: Some(1),
        duration_ms: 215_000,
        sample_rate: 44100,
        bit_depth: 16,
        format: "FLAC".to_string(),
        file_size: 15_000_000,
        modified_time: 1000,
        thumbnail_path: None,
        has_lyrics: false,
        play_count: 0,
        last_played_at: None,
        is_active: true,
    };
    let tid1 = db.upsert_track(&tr1).unwrap();

    let tr2 = TrackRecord {
        id: 0,
        folder_id: fid,
        path: "D:\\Lossless\\song2.flac".to_string(),
        title: "Song Two".to_string(),
        artist: "Band B".to_string(),
        album: "Second Album".to_string(),
        album_artist: None,
        genre: None,
        year: None,
        track_number: Some(2),
        disc_number: Some(1),
        duration_ms: 190_000,
        sample_rate: 96000,
        bit_depth: 24,
        format: "FLAC".to_string(),
        file_size: 25_000_000,
        modified_time: 2000,
        thumbnail_path: None,
        has_lyrics: false,
        play_count: 0,
        last_played_at: None,
        is_active: true,
    };
    let tid2 = db.upsert_track(&tr2).unwrap();

    let pid = PlaylistManager::create_playlist(&db, "Chill Vibes", Some("Evening session"), None).unwrap();
    PlaylistManager::add_track(&db, pid, tid1).unwrap();
    PlaylistManager::add_track(&db, pid, tid2).unwrap();

    let tracks = PlaylistManager::get_playlist_tracks(&db, pid).unwrap();
    assert_eq!(tracks.len(), 2);
    assert_eq!(tracks[0].track.title, "Song One");
    assert_eq!(tracks[1].track.title, "Song Two");

    // Export to M3U8
    let temp_m3u8 = std::env::temp_dir().join("shrimp_test_playlist.m3u8");
    let exported_count = PlaylistManager::export_m3u8(&db, pid, &temp_m3u8).unwrap();
    assert_eq!(exported_count, 2);

    let m3u8_content = std::fs::read_to_string(&temp_m3u8).unwrap();
    assert!(m3u8_content.starts_with("#EXTM3U"));
    assert!(m3u8_content.contains("#EXTINF:215,Band A - Song One"));
    assert!(m3u8_content.contains("D:\\Lossless\\song1.flac"));

    // Import from M3U8 into a new playlist
    let (imported_pid, imported_count) = PlaylistManager::import_m3u8(&db, &temp_m3u8, Some("Imported Chill")).unwrap();
    assert_eq!(imported_count, 2);

    let imported_tracks = PlaylistManager::get_playlist_tracks(&db, imported_pid).unwrap();
    assert_eq!(imported_tracks.len(), 2);
    assert_eq!(imported_tracks[0].track.title, "Song One");

    let _ = std::fs::remove_file(temp_m3u8);
}

#[test]
fn test_queue_shuffle_and_safety_tone() {
    let mut queue = PlaybackQueue::new();

    let mut tracks = Vec::new();
    for i in 1..=10 {
        tracks.push(TrackRecord {
            id: i,
            folder_id: if i <= 5 { 1 } else { 2 },
            path: format!("D:\\Music\\track_{}.flac", i),
            title: format!("Track {}", i),
            artist: "Artist".to_string(),
            album: "Album".to_string(),
            album_artist: None,
            genre: None,
            year: None,
            track_number: Some(i as i32),
            disc_number: Some(1),
            duration_ms: 180_000,
            sample_rate: 44100,
            bit_depth: 16,
            format: "FLAC".to_string(),
            file_size: 10_000_000,
            modified_time: 1000,
            thumbnail_path: None,
            has_lyrics: false,
            play_count: (i * 10) as u64, // higher id has higher play_count for Favffle
            last_played_at: None,
            is_active: true,
        });
    }

    queue.set_items(tracks, Some(0));
    assert_eq!(queue.current_track().unwrap().title, "Track 1");

    // 1. Sequential next
    let next_tr = queue.next().unwrap();
    assert_eq!(next_tr.title, "Track 2");

    // 2. Loop One repeats current
    queue.set_loop_mode(LoopMode::One);
    assert_eq!(queue.next().unwrap().title, "Track 2");

    // 3. Shuffle with ReducedRepeat
    queue.set_loop_mode(LoopMode::All);
    queue.set_shuffle(true);
    queue.set_shuffle_style(ShuffleStyle::ReducedRepeat);

    let mut played_ids = Vec::new();
    for _ in 0..15 {
        if let Some(t) = queue.next() {
            played_ids.push(t.id);
        }
    }
    assert_eq!(played_ids.len(), 15);

    // 4. Strict stop on folder disable
    // Active track has folder_id 1 or 2
    let cur_folder = queue.current_track().unwrap().folder_id;
    let stopped = queue.remove_folder_tracks(cur_folder);
    assert!(stopped, "Should signal stop because currently playing track's folder was removed");

    // 5. Test Hearing Safety Chime generator
    let chime_samples = SafetyTestSignal::generate_10s_chime(48000, 2);
    // 10s * 48000 * 2 = 960000 samples
    assert_eq!(chime_samples.len(), 960_000);

    // Peak amplitude must not exceed safe -18 dBFS threshold (+ slight margin)
    let max_amp = chime_samples.iter().cloned().fold(0.0_f32, |m, s| m.max(s.abs()));
    assert!(max_amp <= 0.13, "Chime peak amplitude ({}) exceeded safe -18 dBFS (~0.126)", max_amp);
}

#[test]
fn test_artwork_downscaling() {
    use image::{Rgb, RgbImage};
    use std::io::Cursor;

    // Create a 1000x1000 dummy RGB image
    let mut img = RgbImage::new(1000, 1000);
    for pixel in img.pixels_mut() {
        *pixel = Rgb([200, 100, 50]);
    }

    let mut buf = Vec::new();
    img.write_to(&mut Cursor::new(&mut buf), image::ImageFormat::Jpeg).unwrap();

    let path_opt = generate_and_save_thumbnail(&buf, "test-album-hash");
    assert!(path_opt.is_some());
    let path_str = path_opt.unwrap();
    let thumb_file = std::path::Path::new(&path_str);
    assert!(thumb_file.exists());

    // Verify dimensions strictly downscaled to 250x250
    let loaded = image::ImageReader::open(thumb_file).unwrap().decode().unwrap();
    assert_eq!(loaded.width(), 250);
    assert_eq!(loaded.height(), 250);

    let _ = clear_thumbnail_cache();
}

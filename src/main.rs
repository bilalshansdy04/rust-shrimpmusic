#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use shrimp_music::audio::{AudioPlayer, DeviceManager};
use shrimp_music::db::{default_db_path, open_database};
use shrimp_music::library::artwork::get_thumbnail_dir;
use shrimp_music::ui::ShrimpMusicApp;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // If not explicitly overridden by user, prefer the software renderer for low RAM footprint (< 50 MB)
    if std::env::var("SLINT_BACKEND").is_err() {
        unsafe {
            std::env::set_var("SLINT_BACKEND", "software");
        }
    }

    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--headless" || a == "-h") {
        let _ = wasapi::initialize_mta();
        println!("=== ShrimpMusic - Headless Status Check ===");

        let dm = DeviceManager::new()?;
        let devices = dm.list_output_devices()?;

        println!("\n[Audio] Detected Audio Output Devices:");
        for dev in &devices {
            let flag = if dev.is_default { " [DEFAULT]" } else { "" };
            println!(" - {}{}", dev.name, flag);
        }

        let _player = AudioPlayer::new()?;
        println!("[Audio] WASAPI Engine Initialized successfully.");

        let db_path = default_db_path();
        let db = open_database(&db_path)?;
        println!("\n[Database] SQLite (WAL Mode) Initialized: {}", db_path.display());

        let folders = db.get_all_folders()?;
        println!("[Library] Monitored Folders: {}", folders.len());
        for f in &folders {
            println!("  - Folder #{} (tracks: {}): {}", f.id, f.total_tracks, f.folder_path);
        }

        let tracks = db.list_tracks(None, shrimp_music::db::SortCriteria::Title, true)?;
        println!("[Library] Total Tracks in DB: {}", tracks.len());
        for t in tracks.iter().take(5) {
            println!("  * Track: {} - {} ({})", t.artist, t.title, t.path);
        }

        let thumb_dir = get_thumbnail_dir();
        println!("[Cache] Thumbnail Directory (250x250): {}", thumb_dir.display());

        println!("\nFase 1, Fase 2 & Fase 3 Core Ready.");
        return Ok(());
    }

    // Launch Slint GUI Application
    let app = ShrimpMusicApp::new()?;
    app.run()?;

    Ok(())
}

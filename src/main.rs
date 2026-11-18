use shrimp_music::audio::{AudioPlayer, DeviceManager};
use shrimp_music::db::{default_db_path, open_database};
use shrimp_music::library::artwork::get_thumbnail_dir;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let _ = wasapi::initialize_mta();
    println!("=== ShrimpMusic Native v2.0 - Headless Core (Fase 1 & Fase 2) ===");

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

    let thumb_dir = get_thumbnail_dir();
    println!("[Cache] Thumbnail Directory (250x250): {}", thumb_dir.display());

    println!("\nFase 2 (Library Scanner, SQLite, Lofty Parser, Playlists & Safety) Ready.");
    Ok(())
}



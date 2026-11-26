use rusqlite::{Connection, Result};

pub fn initialize_database(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        r#"
        PRAGMA journal_mode = WAL;
        PRAGMA synchronous = NORMAL;
        PRAGMA foreign_keys = ON;
        PRAGMA cache_size = -4000;
        PRAGMA temp_store = MEMORY;

        CREATE TABLE IF NOT EXISTS library_folders (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            folder_path TEXT NOT NULL UNIQUE,
            is_enabled INTEGER NOT NULL DEFAULT 1,
            last_scanned_at INTEGER,
            total_tracks INTEGER DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS tracks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            folder_id INTEGER NOT NULL REFERENCES library_folders(id) ON DELETE CASCADE,
            path TEXT NOT NULL UNIQUE,
            title TEXT NOT NULL,
            artist TEXT NOT NULL,
            album TEXT NOT NULL,
            album_artist TEXT,
            genre TEXT,
            year INTEGER,
            track_number INTEGER,
            disc_number INTEGER,
            duration_ms INTEGER NOT NULL,
            sample_rate INTEGER NOT NULL,
            bit_depth INTEGER NOT NULL,
            format TEXT NOT NULL,
            file_size INTEGER NOT NULL,
            modified_time INTEGER NOT NULL,
            thumbnail_path TEXT,
            has_lyrics INTEGER NOT NULL DEFAULT 0,
            play_count INTEGER NOT NULL DEFAULT 0,
            last_played_at INTEGER,
            waveform_data BLOB,
            is_active INTEGER NOT NULL DEFAULT 1
        );

        CREATE TABLE IF NOT EXISTS lyrics (
            track_id INTEGER PRIMARY KEY REFERENCES tracks(id) ON DELETE CASCADE,
            is_synced INTEGER NOT NULL DEFAULT 0,
            raw_text TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS playlists (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            description TEXT,
            custom_artwork_path TEXT,
            created_at INTEGER NOT NULL,
            updated_at INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS playlist_tracks (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            playlist_id INTEGER NOT NULL REFERENCES playlists(id) ON DELETE CASCADE,
            track_id INTEGER NOT NULL REFERENCES tracks(id) ON DELETE CASCADE,
            sort_order INTEGER NOT NULL
        );

        CREATE TABLE IF NOT EXISTS user_preferences (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        CREATE INDEX IF NOT EXISTS idx_tracks_folder ON tracks(folder_id);
        CREATE INDEX IF NOT EXISTS idx_tracks_artist ON tracks(artist);
        CREATE INDEX IF NOT EXISTS idx_tracks_album ON tracks(album);
        CREATE INDEX IF NOT EXISTS idx_tracks_is_active ON tracks(is_active);
        CREATE INDEX IF NOT EXISTS idx_playlist_tracks_order ON playlist_tracks(playlist_id, sort_order);
        "#,
    )?;

    // Safe migration for waveform_data on existing DBs
    let _ = conn.execute("ALTER TABLE tracks ADD COLUMN waveform_data BLOB", []);

    // One-time invalidation of legacy max-peak waveforms so tracks regenerate with dynamic RMS
    let wf_ver: Option<String> = conn
        .query_row(
            "SELECT value FROM user_preferences WHERE key = 'waveform_algo_version'",
            [],
            |row| row.get(0),
        )
        .ok();
    if wf_ver.as_deref() != Some("2") {
        let _ = conn.execute("UPDATE tracks SET waveform_data = NULL", []);
        let _ = conn.execute(
            "INSERT INTO user_preferences (key, value) VALUES ('waveform_algo_version', '2')
             ON CONFLICT(key) DO UPDATE SET value = '2'",
            [],
        );
    }

    Ok(())
}

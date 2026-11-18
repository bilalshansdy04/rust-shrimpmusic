use std::fs::File;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;

use crate::db::models::{Playlist, PlaylistTrackItem};
use crate::db::Database;

pub struct PlaylistManager;

impl PlaylistManager {
    pub fn create_playlist(
        db: &Database,
        name: &str,
        description: Option<&str>,
        custom_artwork_path: Option<&str>,
    ) -> rusqlite::Result<i64> {
        db.create_playlist(name, description, custom_artwork_path)
    }

    pub fn delete_playlist(db: &Database, playlist_id: i64) -> rusqlite::Result<()> {
        db.delete_playlist(playlist_id)
    }

    pub fn get_playlists(db: &Database) -> rusqlite::Result<Vec<Playlist>> {
        db.get_playlists()
    }

    pub fn get_playlist_tracks(db: &Database, playlist_id: i64) -> rusqlite::Result<Vec<PlaylistTrackItem>> {
        db.get_playlist_tracks(playlist_id)
    }

    pub fn add_track(db: &Database, playlist_id: i64, track_id: i64) -> rusqlite::Result<i64> {
        db.add_track_to_playlist(playlist_id, track_id)
    }

    pub fn remove_track(db: &Database, playlist_id: i64, track_id: i64) -> rusqlite::Result<()> {
        db.remove_track_from_playlist(playlist_id, track_id)
    }

    /// Exports playlist to Universal Extended UTF-8 M3U8 format
    pub fn export_m3u8(
        db: &Database,
        playlist_id: i64,
        target_path: &Path,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let items = db.get_playlist_tracks(playlist_id)?;
        let mut file = File::create(target_path)?;

        writeln!(file, "#EXTM3U")?;

        let mut count = 0;
        for item in items {
            let tr = &item.track;
            let duration_sec = tr.duration_ms / 1000;
            writeln!(file, "#EXTINF:{},{} - {}", duration_sec, tr.artist, tr.title)?;
            writeln!(file, "{}", tr.path)?;
            count += 1;
        }

        Ok(count)
    }

    /// Imports a playlist from Universal Extended UTF-8 M3U8 format
    pub fn import_m3u8(
        db: &Database,
        source_path: &Path,
        playlist_name_override: Option<&str>,
    ) -> Result<(i64, usize), Box<dyn std::error::Error + Send + Sync>> {
        let file = File::open(source_path)?;
        let reader = BufReader::new(file);

        let default_name = source_path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Imported Playlist");
        let playlist_name = playlist_name_override.unwrap_or(default_name);

        let playlist_id = db.create_playlist(playlist_name, Some("Imported from M3U8"), None)?;

        let mut tracks_added = 0;
        for line in reader.lines() {
            let line = line?;
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') {
                continue;
            }

            // Path line
            if let Ok(Some(track)) = db.get_track_by_path(trimmed) {
                let _ = db.add_track_to_playlist(playlist_id, track.id);
                tracks_added += 1;
            }
        }

        Ok((playlist_id, tracks_added))
    }
}

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use walkdir::WalkDir;

use crate::db::models::TrackRecord;
use crate::db::Database;
use super::artwork::generate_and_save_thumbnail;
use super::metadata::extract_metadata;

#[derive(Debug, Clone)]
pub enum ScanProgress {
    Started { total_estimated: usize },
    FileScanned { current: usize, total: usize, path: String },
    FolderCompleted { folder_id: i64, added: usize, updated: usize, unchanged: usize, removed: usize },
    Completed { total_tracks: usize },
    Error { message: String },
}

pub struct Scanner {
    supported_extensions: Vec<&'static str>,
}

impl Default for Scanner {
    fn default() -> Self {
        Self {
            supported_extensions: vec!["flac", "mp3", "m4a", "alac", "aac", "wav", "ogg"],
        }
    }
}

impl Scanner {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_supported_extension(&self, path: &Path) -> bool {
        path.extension()
            .and_then(|s| s.to_str())
            .map(|ext| {
                let lower = ext.to_lowercase();
                self.supported_extensions.iter().any(|&e| e == lower)
            })
            .unwrap_or(false)
    }

    pub fn scan_folder(
        &self,
        db: &Database,
        folder_id: i64,
        folder_path: &Path,
        cancel_token: Option<&Arc<AtomicBool>>,
        progress_tx: Option<&crossbeam_channel::Sender<ScanProgress>>,
    ) -> Result<(usize, usize, usize, usize), Box<dyn std::error::Error + Send + Sync>> {
        self.scan_folder_with_options(
            db,
            folder_id,
            folder_path,
            cancel_token,
            progress_tx,
            false,
        )
    }

    pub fn scan_folder_with_options(
        &self,
        db: &Database,
        folder_id: i64,
        folder_path: &Path,
        cancel_token: Option<&Arc<AtomicBool>>,
        progress_tx: Option<&crossbeam_channel::Sender<ScanProgress>>,
        force_refresh: bool,
    ) -> Result<(usize, usize, usize, usize), Box<dyn std::error::Error + Send + Sync>> {
        let mut audio_files = Vec::new();
        for entry in WalkDir::new(folder_path).follow_links(true).into_iter().filter_map(|e| e.ok()) {
            if let Some(token) = cancel_token {
                if token.load(Ordering::Relaxed) {
                    return Ok((0, 0, 0, 0));
                }
            }

            let path = entry.path();
            if path.is_file() && self.is_supported_extension(path) {
                audio_files.push(path.to_path_buf());
            }
        }

        let total = audio_files.len();
        if let Some(tx) = progress_tx {
            let _ = tx.send(ScanProgress::Started { total_estimated: total });
        }

        let mut added = 0;
        let mut updated = 0;
        let mut unchanged = 0;
        let mut valid_paths = Vec::with_capacity(total);

        for (idx, path) in audio_files.into_iter().enumerate() {
            if let Some(token) = cancel_token {
                if token.load(Ordering::Relaxed) {
                    break;
                }
            }

            let path_str = path.to_string_lossy().to_string();
            valid_paths.push(path_str.clone());

            let fs_meta = match std::fs::metadata(&path) {
                Ok(m) => m,
                Err(_) => continue,
            };
            let file_size = fs_meta.len();
            let modified_time = fs_meta
                .modified()
                .ok()
                .and_then(|m| m.duration_since(std::time::UNIX_EPOCH).ok())
                .map(|d| d.as_secs() as i64)
                .unwrap_or(0);

            // Fast skip check: if not force_refresh and already in DB with same file size and mod time
            // (and no newer external .lrc file), skip heavy metadata extraction!
            if !force_refresh {
                if let Ok(Some(existing)) = db.get_track_by_path(&path_str) {
                    let lrc_changed = {
                        let lrc_path = path.with_extension("lrc");
                        if lrc_path.exists() {
                            let lrc_mtime = std::fs::metadata(&lrc_path)
                                .ok()
                                .and_then(|m| m.modified().ok())
                                .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                                .map(|d| d.as_secs() as i64)
                                .unwrap_or(0);
                            !existing.has_lyrics || lrc_mtime > existing.modified_time
                        } else {
                            false
                        }
                    };

                    if !lrc_changed
                        && existing.file_size == file_size
                        && existing.modified_time == modified_time
                    {
                        if existing.folder_id != folder_id {
                            let _ = db.conn().execute(
                                "UPDATE tracks SET folder_id = ?1 WHERE path = ?2",
                                rusqlite::params![folder_id, &path_str],
                            );
                        }
                        unchanged += 1;
                        if let Some(tx) = progress_tx {
                            let _ = tx.send(ScanProgress::FileScanned {
                                current: idx + 1,
                                total,
                                path: path_str,
                            });
                        }
                        continue;
                    }
                }
            }

            // Extract metadata with Lofty
            match extract_metadata(&path) {
                Ok(meta) => {
                    let thumb_path = if let Some(cover_data) = &meta.cover_art {
                        let key = format!("{}-{}-{}", meta.album, meta.artist, meta.title);
                        generate_and_save_thumbnail(cover_data, &key)
                    } else {
                        None
                    };

                    let record = TrackRecord {
                        id: 0,
                        folder_id,
                        path: path_str.clone(),
                        title: meta.title,
                        artist: meta.artist,
                        album: meta.album,
                        album_artist: meta.album_artist,
                        genre: meta.genre,
                        year: meta.year,
                        track_number: meta.track_number,
                        disc_number: meta.disc_number,
                        duration_ms: meta.duration_ms,
                        sample_rate: meta.sample_rate,
                        bit_depth: meta.bit_depth,
                        format: meta.format,
                        file_size,
                        modified_time,
                        thumbnail_path: thumb_path,
                        has_lyrics: meta.lyrics.is_some(),
                        play_count: 0,
                        last_played_at: None,
                        waveform: None,
                        is_active: true,
                    };

                    let track_id = db.upsert_track(&record)?;

                    if let Some(lyrics_text) = meta.lyrics {
                        let _ = db.set_lyrics(track_id, meta.is_synced_lyrics, &lyrics_text);
                    }

                    if db.get_track_by_path(&path_str)?.map(|t| t.id == track_id).unwrap_or(false) {
                        added += 1;
                    } else {
                        updated += 1;
                    }
                }
                Err(_) => {
                    // Skip files that fail parsing
                }
            }

            if let Some(tx) = progress_tx {
                let _ = tx.send(ScanProgress::FileScanned {
                    current: idx + 1,
                    total,
                    path: path_str,
                });
            }
        }

        let removed = db.prune_deleted_tracks(folder_id, &valid_paths).unwrap_or(0);
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        let total_tracks = (added + updated + unchanged) as i64;
        let _ = db.update_folder_scan_stats(folder_id, total_tracks, now);

        if let Some(tx) = progress_tx {
            let _ = tx.send(ScanProgress::FolderCompleted {
                folder_id,
                added,
                updated,
                unchanged,
                removed,
            });
        }

        Ok((added, updated, unchanged, removed))
    }

    pub fn scan_all_enabled_folders(
        &self,
        db: &Database,
        cancel_token: Option<&Arc<AtomicBool>>,
        progress_tx: Option<&crossbeam_channel::Sender<ScanProgress>>,
    ) -> Result<usize, Box<dyn std::error::Error + Send + Sync>> {
        let folders = db.get_all_folders()?;
        let mut total_scanned = 0;

        for folder in folders {
            if !folder.is_enabled {
                continue;
            }

            let path = PathBuf::from(&folder.folder_path);
            if path.exists() {
                let (a, u, un, _) = self.scan_folder(db, folder.id, &path, cancel_token, progress_tx)?;
                total_scanned += a + u + un;
            }
        }

        if let Some(tx) = progress_tx {
            let _ = tx.send(ScanProgress::Completed { total_tracks: total_scanned });
        }

        Ok(total_scanned)
    }
}

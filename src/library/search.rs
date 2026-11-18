use rusqlite::params;
use crate::db::models::TrackRecord;
use crate::db::Database;

#[derive(Debug, Clone)]
pub struct LyricSearchHit {
    pub track_id: i64,
    pub track_title: String,
    pub track_artist: String,
    pub timestamp_ms: i64,
    pub snippet: String,
}

#[derive(Debug, Clone, Default)]
pub struct SearchResults {
    pub tracks: Vec<TrackRecord>,
    pub lyrics: Vec<LyricSearchHit>,
}

pub struct SearchEngine;

impl SearchEngine {
    pub fn search(db: &Database, query: &str) -> rusqlite::Result<SearchResults> {
        let trimmed = query.trim();
        if trimmed.is_empty() {
            return Ok(SearchResults::default());
        }

        if let Some(rest) = trimmed.strip_prefix("lirik:").or_else(|| trimmed.strip_prefix("lyric:")) {
            let hits = Self::search_lyrics(db, rest.trim())?;
            return Ok(SearchResults {
                tracks: Vec::new(),
                lyrics: hits,
            });
        }

        if let Some(rest) = trimmed.strip_prefix("artist:") {
            let tracks = Self::search_by_artist(db, rest.trim())?;
            return Ok(SearchResults {
                tracks,
                lyrics: Vec::new(),
            });
        }

        if let Some(rest) = trimmed.strip_prefix("album:") {
            let tracks = Self::search_by_album(db, rest.trim())?;
            return Ok(SearchResults {
                tracks,
                lyrics: Vec::new(),
            });
        }

        // Default query: Search tracks (Group 1) and lyrics (Group 2)
        let tracks = Self::search_tracks(db, trimmed)?;
        let lyrics = Self::search_lyrics(db, trimmed)?;

        Ok(SearchResults { tracks, lyrics })
    }

    fn search_tracks(db: &Database, query: &str) -> rusqlite::Result<Vec<TrackRecord>> {
        let pattern = format!("%{}%", query);
        let sql = r#"
            SELECT t.id, t.folder_id, t.path, t.title, t.artist, t.album, t.album_artist,
                   t.genre, t.year, t.track_number, t.disc_number, t.duration_ms,
                   t.sample_rate, t.bit_depth, t.format, t.file_size, t.modified_time,
                   t.thumbnail_path, t.has_lyrics, t.play_count, t.last_played_at, t.is_active
            FROM tracks t
            JOIN library_folders f ON t.folder_id = f.id
            WHERE t.is_active = 1 AND f.is_enabled = 1 AND (
                t.title LIKE ?1 ESCAPE '\' OR
                t.artist LIKE ?1 ESCAPE '\' OR
                t.album LIKE ?1 ESCAPE '\'
            )
            ORDER BY t.title COLLATE NOCASE ASC
            LIMIT 50
        "#;

        let mut stmt = db.conn().prepare(sql)?;
        let rows = stmt.query_map(params![pattern], |row| {
            Ok(TrackRecord {
                id: row.get(0)?,
                folder_id: row.get(1)?,
                path: row.get(2)?,
                title: row.get(3)?,
                artist: row.get(4)?,
                album: row.get(5)?,
                album_artist: row.get(6)?,
                genre: row.get(7)?,
                year: row.get(8)?,
                track_number: row.get(9)?,
                disc_number: row.get(10)?,
                duration_ms: row.get(11)?,
                sample_rate: row.get::<_, u32>(12)?,
                bit_depth: row.get::<_, u32>(13)?,
                format: row.get(14)?,
                file_size: row.get::<_, i64>(15)? as u64,
                modified_time: row.get(16)?,
                thumbnail_path: row.get(17)?,
                has_lyrics: row.get::<_, i64>(18)? == 1,
                play_count: row.get::<_, i64>(19)? as u64,
                last_played_at: row.get(20)?,
                is_active: row.get::<_, i64>(21)? == 1,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    fn search_by_artist(db: &Database, query: &str) -> rusqlite::Result<Vec<TrackRecord>> {
        let pattern = format!("%{}%", query);
        let sql = r#"
            SELECT t.id, t.folder_id, t.path, t.title, t.artist, t.album, t.album_artist,
                   t.genre, t.year, t.track_number, t.disc_number, t.duration_ms,
                   t.sample_rate, t.bit_depth, t.format, t.file_size, t.modified_time,
                   t.thumbnail_path, t.has_lyrics, t.play_count, t.last_played_at, t.is_active
            FROM tracks t
            JOIN library_folders f ON t.folder_id = f.id
            WHERE t.is_active = 1 AND f.is_enabled = 1 AND t.artist LIKE ?1 ESCAPE '\'
            ORDER BY t.album COLLATE NOCASE ASC, t.track_number ASC
            LIMIT 50
        "#;

        let mut stmt = db.conn().prepare(sql)?;
        let rows = stmt.query_map(params![pattern], |row| {
            Ok(TrackRecord {
                id: row.get(0)?,
                folder_id: row.get(1)?,
                path: row.get(2)?,
                title: row.get(3)?,
                artist: row.get(4)?,
                album: row.get(5)?,
                album_artist: row.get(6)?,
                genre: row.get(7)?,
                year: row.get(8)?,
                track_number: row.get(9)?,
                disc_number: row.get(10)?,
                duration_ms: row.get(11)?,
                sample_rate: row.get::<_, u32>(12)?,
                bit_depth: row.get::<_, u32>(13)?,
                format: row.get(14)?,
                file_size: row.get::<_, i64>(15)? as u64,
                modified_time: row.get(16)?,
                thumbnail_path: row.get(17)?,
                has_lyrics: row.get::<_, i64>(18)? == 1,
                play_count: row.get::<_, i64>(19)? as u64,
                last_played_at: row.get(20)?,
                is_active: row.get::<_, i64>(21)? == 1,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    fn search_by_album(db: &Database, query: &str) -> rusqlite::Result<Vec<TrackRecord>> {
        let pattern = format!("%{}%", query);
        let sql = r#"
            SELECT t.id, t.folder_id, t.path, t.title, t.artist, t.album, t.album_artist,
                   t.genre, t.year, t.track_number, t.disc_number, t.duration_ms,
                   t.sample_rate, t.bit_depth, t.format, t.file_size, t.modified_time,
                   t.thumbnail_path, t.has_lyrics, t.play_count, t.last_played_at, t.is_active
            FROM tracks t
            JOIN library_folders f ON t.folder_id = f.id
            WHERE t.is_active = 1 AND f.is_enabled = 1 AND t.album LIKE ?1 ESCAPE '\'
            ORDER BY t.track_number ASC
            LIMIT 50
        "#;

        let mut stmt = db.conn().prepare(sql)?;
        let rows = stmt.query_map(params![pattern], |row| {
            Ok(TrackRecord {
                id: row.get(0)?,
                folder_id: row.get(1)?,
                path: row.get(2)?,
                title: row.get(3)?,
                artist: row.get(4)?,
                album: row.get(5)?,
                album_artist: row.get(6)?,
                genre: row.get(7)?,
                year: row.get(8)?,
                track_number: row.get(9)?,
                disc_number: row.get(10)?,
                duration_ms: row.get(11)?,
                sample_rate: row.get::<_, u32>(12)?,
                bit_depth: row.get::<_, u32>(13)?,
                format: row.get(14)?,
                file_size: row.get::<_, i64>(15)? as u64,
                modified_time: row.get(16)?,
                thumbnail_path: row.get(17)?,
                has_lyrics: row.get::<_, i64>(18)? == 1,
                play_count: row.get::<_, i64>(19)? as u64,
                last_played_at: row.get(20)?,
                is_active: row.get::<_, i64>(21)? == 1,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    fn search_lyrics(db: &Database, query: &str) -> rusqlite::Result<Vec<LyricSearchHit>> {
        let pattern = format!("%{}%", query);
        let sql = r#"
            SELECT l.track_id, t.title, t.artist, l.is_synced, l.raw_text
            FROM lyrics l
            JOIN tracks t ON l.track_id = t.id
            JOIN library_folders f ON t.folder_id = f.id
            WHERE t.is_active = 1 AND f.is_enabled = 1 AND l.raw_text LIKE ?1 ESCAPE '\'
            LIMIT 30
        "#;

        let mut stmt = db.conn().prepare(sql)?;
        let rows = stmt.query_map(params![pattern], |row| {
            let track_id: i64 = row.get(0)?;
            let title: String = row.get(1)?;
            let artist: String = row.get(2)?;
            let is_synced: bool = row.get::<_, i64>(3)? == 1;
            let raw_text: String = row.get(4)?;
            Ok((track_id, title, artist, is_synced, raw_text))
        })?;

        let query_lower = query.to_lowercase();
        let mut hits = Vec::new();

        for r in rows {
            let (track_id, title, artist, is_synced, raw_text) = r?;
            if is_synced {
                let lines = crate::library::metadata::parse_lrc_lines(&raw_text);
                for line in lines {
                    if line.text.to_lowercase().contains(&query_lower) {
                        hits.push(LyricSearchHit {
                            track_id,
                            track_title: title.clone(),
                            track_artist: artist.clone(),
                            timestamp_ms: line.timestamp_ms,
                            snippet: line.text,
                        });
                        break; // Top matching line per song
                    }
                }
            } else {
                for line in raw_text.lines() {
                    if line.to_lowercase().contains(&query_lower) {
                        hits.push(LyricSearchHit {
                            track_id,
                            track_title: title.clone(),
                            track_artist: artist.clone(),
                            timestamp_ms: -1,
                            snippet: line.trim().to_string(),
                        });
                        break;
                    }
                }
            }
        }

        Ok(hits)
    }
}

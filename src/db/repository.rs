use rusqlite::{params, Connection, Result};

use super::models::{LibraryFolder, LyricLine, LyricsRecord, Playlist, PlaylistTrackItem, SortCriteria, TrackRecord};

pub struct Database {
    conn: Connection,
}

impl Database {
    pub fn new(conn: Connection) -> Result<Self> {
        super::schema::initialize_database(&conn)?;
        Ok(Self { conn })
    }

    pub fn conn(&self) -> &Connection {
        &self.conn
    }

    pub fn conn_mut(&mut self) -> &mut Connection {
        &mut self.conn
    }

    // --- Folders ---

    pub fn add_folder(&self, path: &str) -> Result<i64> {
        self.conn.execute(
            "INSERT INTO library_folders (folder_path, is_enabled) VALUES (?1, 1)
             ON CONFLICT(folder_path) DO UPDATE SET is_enabled = 1",
            params![path],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn remove_folder(&self, folder_id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM library_folders WHERE id = ?1", params![folder_id])?;
        Ok(())
    }

    pub fn set_folder_enabled(&self, folder_id: i64, enabled: bool) -> Result<()> {
        self.conn.execute(
            "UPDATE library_folders SET is_enabled = ?1 WHERE id = ?2",
            params![if enabled { 1 } else { 0 }, folder_id],
        )?;
        Ok(())
    }

    pub fn get_all_folders(&self) -> Result<Vec<LibraryFolder>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, folder_path, is_enabled, last_scanned_at, total_tracks FROM library_folders ORDER BY id ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(LibraryFolder {
                id: row.get(0)?,
                folder_path: row.get(1)?,
                is_enabled: row.get::<_, i64>(2)? == 1,
                last_scanned_at: row.get(3)?,
                total_tracks: row.get(4)?,
            })
        })?;

        let mut folders = Vec::new();
        for f in rows {
            folders.push(f?);
        }
        Ok(folders)
    }

    pub fn get_folder_by_id(&self, folder_id: i64) -> Result<Option<LibraryFolder>> {
        let mut stmt = self.conn.prepare(
            "SELECT id, folder_path, is_enabled, last_scanned_at, total_tracks FROM library_folders WHERE id = ?1",
        )?;
        let mut rows = stmt.query_map(params![folder_id], |row| {
            Ok(LibraryFolder {
                id: row.get(0)?,
                folder_path: row.get(1)?,
                is_enabled: row.get::<_, i64>(2)? == 1,
                last_scanned_at: row.get(3)?,
                total_tracks: row.get(4)?,
            })
        })?;

        if let Some(res) = rows.next() {
            Ok(Some(res?))
        } else {
            Ok(None)
        }
    }

    pub fn update_folder_scan_stats(&self, folder_id: i64, total_tracks: i64, scanned_at: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE library_folders SET total_tracks = ?1, last_scanned_at = ?2 WHERE id = ?3",
            params![total_tracks, scanned_at, folder_id],
        )?;
        Ok(())
    }

    // --- Tracks ---

    pub fn upsert_track(&self, track: &TrackRecord) -> Result<i64> {
        self.conn.execute(
            r#"
            INSERT INTO tracks (
                folder_id, path, title, artist, album, album_artist, genre, year,
                track_number, disc_number, duration_ms, sample_rate, bit_depth,
                format, file_size, modified_time, thumbnail_path, has_lyrics, play_count, is_active
            ) VALUES (
                ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, 1
            )
            ON CONFLICT(path) DO UPDATE SET
                folder_id = excluded.folder_id,
                title = excluded.title,
                artist = excluded.artist,
                album = excluded.album,
                album_artist = excluded.album_artist,
                genre = excluded.genre,
                year = excluded.year,
                track_number = excluded.track_number,
                disc_number = excluded.disc_number,
                duration_ms = excluded.duration_ms,
                sample_rate = excluded.sample_rate,
                bit_depth = excluded.bit_depth,
                format = excluded.format,
                file_size = excluded.file_size,
                modified_time = excluded.modified_time,
                thumbnail_path = COALESCE(excluded.thumbnail_path, tracks.thumbnail_path),
                has_lyrics = excluded.has_lyrics,
                is_active = 1
            "#,
            params![
                track.folder_id,
                track.path,
                track.title,
                track.artist,
                track.album,
                track.album_artist,
                track.genre,
                track.year,
                track.track_number,
                track.disc_number,
                track.duration_ms,
                track.sample_rate,
                track.bit_depth,
                track.format,
                track.file_size as i64,
                track.modified_time,
                track.thumbnail_path,
                if track.has_lyrics { 1 } else { 0 },
                track.play_count as i64,
            ],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn get_track_by_path(&self, path: &str) -> Result<Option<TrackRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, folder_id, path, title, artist, album, album_artist, genre, year,
                   track_number, disc_number, duration_ms, sample_rate, bit_depth, format,
                   file_size, modified_time, thumbnail_path, has_lyrics, play_count,
                   last_played_at, is_active
            FROM tracks WHERE path = ?1
            "#,
        )?;
        let mut rows = stmt.query_map(params![path], |row| Self::map_track_row(row))?;
        if let Some(r) = rows.next() {
            Ok(Some(r?))
        } else {
            Ok(None)
        }
    }

    pub fn get_track_by_id(&self, id: i64) -> Result<Option<TrackRecord>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT id, folder_id, path, title, artist, album, album_artist, genre, year,
                   track_number, disc_number, duration_ms, sample_rate, bit_depth, format,
                   file_size, modified_time, thumbnail_path, has_lyrics, play_count,
                   last_played_at, is_active
            FROM tracks WHERE id = ?1
            "#,
        )?;
        let mut rows = stmt.query_map(params![id], |row| Self::map_track_row(row))?;
        if let Some(r) = rows.next() {
            Ok(Some(r?))
        } else {
            Ok(None)
        }
    }

    pub fn list_tracks(
        &self,
        folder_scope: Option<i64>,
        sort: SortCriteria,
        ascending: bool,
    ) -> Result<Vec<TrackRecord>> {
        let dir = if ascending { "ASC" } else { "DESC" };
        let order_clause = match sort {
            SortCriteria::Title => format!("t.title COLLATE NOCASE {}", dir),
            SortCriteria::Artist => format!("t.artist COLLATE NOCASE {}, t.album COLLATE NOCASE ASC, t.track_number ASC", dir),
            SortCriteria::Album => format!("t.album COLLATE NOCASE {}, t.disc_number ASC, t.track_number ASC", dir),
            SortCriteria::TrackNumber => format!("t.disc_number {}, t.track_number {}", dir, dir),
            SortCriteria::Duration => format!("t.duration_ms {}", dir),
            SortCriteria::DateAdded => format!("t.modified_time {}", dir),
            SortCriteria::AudioSpecs => format!("t.sample_rate {}, t.bit_depth {}", dir, dir),
        };

        let (where_clause, params_vec): (String, Vec<rusqlite::types::Value>) = match folder_scope {
            Some(fid) => (
                "WHERE t.folder_id = ?1 AND t.is_active = 1 AND f.is_enabled = 1".to_string(),
                vec![fid.into()],
            ),
            None => (
                "WHERE t.is_active = 1 AND f.is_enabled = 1".to_string(),
                Vec::new(),
            ),
        };

        let sql = format!(
            r#"
            SELECT t.id, t.folder_id, t.path, t.title, t.artist, t.album, t.album_artist,
                   t.genre, t.year, t.track_number, t.disc_number, t.duration_ms,
                   t.sample_rate, t.bit_depth, t.format, t.file_size, t.modified_time,
                   t.thumbnail_path, t.has_lyrics, t.play_count, t.last_played_at, t.is_active
            FROM tracks t
            JOIN library_folders f ON t.folder_id = f.id
            {}
            ORDER BY {}
            "#,
            where_clause, order_clause
        );

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params_vec), |row| {
            Self::map_track_row(row)
        })?;

        let mut tracks = Vec::new();
        for tr in rows {
            tracks.push(tr?);
        }
        Ok(tracks)
    }

    pub fn get_artists(&self, folder_scope: Option<i64>) -> Result<Vec<(String, usize)>> {
        let (where_clause, params_vec): (String, Vec<rusqlite::types::Value>) = match folder_scope {
            Some(fid) => (
                "WHERE t.folder_id = ?1 AND t.is_active = 1 AND f.is_enabled = 1".to_string(),
                vec![fid.into()],
            ),
            None => (
                "WHERE t.is_active = 1 AND f.is_enabled = 1".to_string(),
                Vec::new(),
            ),
        };

        let sql = format!(
            r#"
            SELECT t.artist, COUNT(t.id) as track_count
            FROM tracks t
            JOIN library_folders f ON t.folder_id = f.id
            {}
            GROUP BY t.artist
            ORDER BY t.artist COLLATE NOCASE ASC
            "#,
            where_clause
        );

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params_vec), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn get_albums(&self, folder_scope: Option<i64>) -> Result<Vec<(String, String, Option<String>, usize)>> {
        let (where_clause, params_vec): (String, Vec<rusqlite::types::Value>) = match folder_scope {
            Some(fid) => (
                "WHERE t.folder_id = ?1 AND t.is_active = 1 AND f.is_enabled = 1".to_string(),
                vec![fid.into()],
            ),
            None => (
                "WHERE t.is_active = 1 AND f.is_enabled = 1".to_string(),
                Vec::new(),
            ),
        };

        let sql = format!(
            r#"
            SELECT t.album, t.artist, t.thumbnail_path, COUNT(t.id) as track_count
            FROM tracks t
            JOIN library_folders f ON t.folder_id = f.id
            {}
            GROUP BY t.album, t.artist
            ORDER BY t.album COLLATE NOCASE ASC
            "#,
            where_clause
        );

        let mut stmt = self.conn.prepare(&sql)?;
        let rows = stmt.query_map(rusqlite::params_from_iter(params_vec), |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, i64>(3)? as usize,
            ))
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn update_track_waveform(&self, track_id: i64, waveform: &[f32]) -> Result<()> {
        let bytes: Vec<u8> = waveform.iter().flat_map(|f| f.to_le_bytes()).collect();
        self.conn.execute(
            "UPDATE tracks SET waveform_data = ?1 WHERE id = ?2",
            params![bytes, track_id],
        )?;
        Ok(())
    }

    pub fn get_track_waveform(&self, track_id: i64) -> Result<Option<Vec<f32>>> {
        let mut stmt = self.conn.prepare("SELECT waveform_data FROM tracks WHERE id = ?1")?;
        let mut rows = stmt.query_map(params![track_id], |row| {
            let blob: Option<Vec<u8>> = row.get(0)?;
            Ok(blob.map(|b| {
                b.chunks_exact(4)
                    .map(|chunk| f32::from_le_bytes(chunk.try_into().unwrap_or([0; 4])))
                    .collect::<Vec<f32>>()
            }))
        })?;

        if let Some(r) = rows.next() {
            Ok(r?)
        } else {
            Ok(None)
        }
    }

    pub fn increment_play_count(&self, track_id: i64, played_at: i64) -> Result<()> {
        self.conn.execute(
            "UPDATE tracks SET play_count = play_count + 1, last_played_at = ?1 WHERE id = ?2",
            params![played_at, track_id],
        )?;
        Ok(())
    }

    pub fn prune_deleted_tracks(&self, folder_id: i64, valid_paths: &[String]) -> Result<usize> {
        if valid_paths.is_empty() {
            let count = self.conn.execute(
                "DELETE FROM tracks WHERE folder_id = ?1",
                params![folder_id],
            )?;
            return Ok(count);
        }

        // Chunking deletion to avoid sqlite parameter limits
        let mut total_deleted = 0;
        let mut stmt = self.conn.prepare("SELECT id, path FROM tracks WHERE folder_id = ?1")?;
        let rows = stmt.query_map(params![folder_id], |row| {
            Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?))
        })?;

        let mut to_delete = Vec::new();
        for r in rows {
            let (id, path) = r?;
            if !valid_paths.contains(&path) {
                to_delete.push(id);
            }
        }

        for id in to_delete {
            total_deleted += self.conn.execute("DELETE FROM tracks WHERE id = ?1", params![id])?;
        }

        Ok(total_deleted)
    }

    // --- Lyrics ---

    pub fn set_lyrics(&self, track_id: i64, is_synced: bool, raw_text: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO lyrics (track_id, is_synced, raw_text) VALUES (?1, ?2, ?3)
             ON CONFLICT(track_id) DO UPDATE SET is_synced = excluded.is_synced, raw_text = excluded.raw_text",
            params![track_id, if is_synced { 1 } else { 0 }, raw_text],
        )?;
        self.conn.execute(
            "UPDATE tracks SET has_lyrics = 1 WHERE id = ?1",
            params![track_id],
        )?;
        Ok(())
    }

    pub fn get_lyrics(&self, track_id: i64) -> Result<Option<LyricsRecord>> {
        let mut stmt = self.conn.prepare("SELECT track_id, is_synced, raw_text FROM lyrics WHERE track_id = ?1")?;
        let mut rows = stmt.query_map(params![track_id], |row| {
            let is_synced = row.get::<_, i64>(1)? == 1;
            let raw_text: String = row.get(2)?;
            let lines = if is_synced {
                crate::library::metadata::parse_lrc_lines(&raw_text)
            } else {
                raw_text
                    .lines()
                    .map(|l| LyricLine {
                        timestamp_ms: -1,
                        text: l.to_string(),
                    })
                    .collect()
            };
            Ok(LyricsRecord {
                track_id: row.get(0)?,
                is_synced,
                raw_text,
                lines,
            })
        })?;

        if let Some(r) = rows.next() {
            Ok(Some(r?))
        } else {
            Ok(None)
        }
    }

    // --- Playlists ---

    pub fn create_playlist(&self, name: &str, description: Option<&str>, custom_artwork_path: Option<&str>) -> Result<i64> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;

        self.conn.execute(
            "INSERT INTO playlists (name, description, custom_artwork_path, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![name, description, custom_artwork_path, now, now],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn update_playlist_artwork(&self, playlist_id: i64, artwork_path: &str) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        self.conn.execute(
            "UPDATE playlists SET custom_artwork_path = ?1, updated_at = ?2 WHERE id = ?3",
            params![artwork_path, now, playlist_id],
        )?;
        Ok(())
    }

    pub fn update_playlist_name(&self, playlist_id: i64, new_name: &str) -> Result<()> {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs() as i64;
        self.conn.execute(
            "UPDATE playlists SET name = ?1, updated_at = ?2 WHERE id = ?3",
            params![new_name, now, playlist_id],
        )?;
        Ok(())
    }

    pub fn delete_playlist(&self, playlist_id: i64) -> Result<()> {
        self.conn.execute("DELETE FROM playlists WHERE id = ?1", params![playlist_id])?;
        Ok(())
    }

    pub fn get_playlists(&self) -> Result<Vec<Playlist>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT p.id, p.name, p.description, p.custom_artwork_path, p.created_at, p.updated_at,
                   COUNT(pt.id) as track_count
            FROM playlists p
            LEFT JOIN playlist_tracks pt ON p.id = pt.playlist_id
            GROUP BY p.id
            ORDER BY p.name COLLATE NOCASE ASC
            "#,
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(Playlist {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                custom_artwork_path: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
                track_count: row.get::<_, i64>(6)? as usize,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn get_playlist(&self, playlist_id: i64) -> Result<Option<Playlist>> {
        let mut stmt = self.conn.prepare(
            r#"
            SELECT p.id, p.name, p.description, p.custom_artwork_path, p.created_at, p.updated_at,
                   COUNT(pt.id) as track_count
            FROM playlists p
            LEFT JOIN playlist_tracks pt ON p.id = pt.playlist_id
            WHERE p.id = ?1
            GROUP BY p.id
            "#,
        )?;
        let mut rows = stmt.query_map(params![playlist_id], |row| {
            Ok(Playlist {
                id: row.get(0)?,
                name: row.get(1)?,
                description: row.get(2)?,
                custom_artwork_path: row.get(3)?,
                created_at: row.get(4)?,
                updated_at: row.get(5)?,
                track_count: row.get::<_, i64>(6)? as usize,
            })
        })?;

        if let Some(r) = rows.next() {
            Ok(Some(r?))
        } else {
            Ok(None)
        }
    }

    pub fn add_track_to_playlist(&self, playlist_id: i64, track_id: i64) -> Result<i64> {
        let mut stmt = self.conn.prepare(
            "SELECT COALESCE(MAX(sort_order), -1) + 1 FROM playlist_tracks WHERE playlist_id = ?1",
        )?;
        let next_order: i64 = stmt.query_row(params![playlist_id], |row| row.get(0))?;

        self.conn.execute(
            "INSERT INTO playlist_tracks (playlist_id, track_id, sort_order) VALUES (?1, ?2, ?3)",
            params![playlist_id, track_id, next_order],
        )?;
        Ok(self.conn.last_insert_rowid())
    }

    pub fn remove_track_from_playlist(&self, playlist_id: i64, track_id: i64) -> Result<()> {
        self.conn.execute(
            "DELETE FROM playlist_tracks WHERE playlist_id = ?1 AND track_id = ?2",
            params![playlist_id, track_id],
        )?;
        Ok(())
    }

    pub fn get_playlist_tracks(&self, playlist_id: i64) -> Result<Vec<PlaylistTrackItem>> {
        let sql = r#"
            SELECT pt.id, pt.playlist_id, pt.track_id, pt.sort_order,
                   t.id, t.folder_id, t.path, t.title, t.artist, t.album, t.album_artist,
                   t.genre, t.year, t.track_number, t.disc_number, t.duration_ms,
                   t.sample_rate, t.bit_depth, t.format, t.file_size, t.modified_time,
                   t.thumbnail_path, t.has_lyrics, t.play_count, t.last_played_at,
                   CASE WHEN f.is_enabled = 1 AND t.is_active = 1 THEN 1 ELSE 0 END as is_active
            FROM playlist_tracks pt
            JOIN tracks t ON pt.track_id = t.id
            JOIN library_folders f ON t.folder_id = f.id
            WHERE pt.playlist_id = ?1
            ORDER BY pt.sort_order ASC
        "#;

        let mut stmt = self.conn.prepare(sql)?;
        let rows = stmt.query_map(params![playlist_id], |row| {
            let pt_id: i64 = row.get(0)?;
            let p_id: i64 = row.get(1)?;
            let t_id: i64 = row.get(2)?;
            let s_order: i64 = row.get(3)?;

            let track = TrackRecord {
                id: row.get(4)?,
                folder_id: row.get(5)?,
                path: row.get(6)?,
                title: row.get(7)?,
                artist: row.get(8)?,
                album: row.get(9)?,
                album_artist: row.get(10)?,
                genre: row.get(11)?,
                year: row.get(12)?,
                track_number: row.get(13)?,
                disc_number: row.get(14)?,
                duration_ms: row.get(15)?,
                sample_rate: row.get::<_, u32>(16)?,
                bit_depth: row.get::<_, u32>(17)?,
                format: row.get(18)?,
                file_size: row.get::<_, i64>(19)? as u64,
                modified_time: row.get(20)?,
                thumbnail_path: row.get(21)?,
                has_lyrics: row.get::<_, i64>(22)? == 1,
                play_count: row.get::<_, i64>(23)? as u64,
                last_played_at: row.get(24)?,
                waveform: None,
                is_active: row.get::<_, i64>(25)? == 1,
            };

            Ok(PlaylistTrackItem {
                id: pt_id,
                playlist_id: p_id,
                track_id: t_id,
                sort_order: s_order,
                track,
            })
        })?;

        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    // --- Preferences ---

    pub fn get_preference(&self, key: &str) -> Result<Option<String>> {
        let mut stmt = self.conn.prepare("SELECT value FROM user_preferences WHERE key = ?1")?;
        let mut rows = stmt.query_map(params![key], |row| row.get(0))?;
        if let Some(r) = rows.next() {
            Ok(Some(r?))
        } else {
            Ok(None)
        }
    }

    pub fn set_preference(&self, key: &str, value: &str) -> Result<()> {
        self.conn.execute(
            "INSERT INTO user_preferences (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    // --- Helper ---

    fn map_track_row(row: &rusqlite::Row) -> rusqlite::Result<TrackRecord> {
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
            waveform: None,
            is_active: row.get::<_, i64>(21)? == 1,
        })
    }
}

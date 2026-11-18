#[derive(Debug, Clone)]
pub struct LibraryFolder {
    pub id: i64,
    pub folder_path: String,
    pub is_enabled: bool,
    pub last_scanned_at: Option<i64>,
    pub total_tracks: i64,
}

#[derive(Debug, Clone)]
pub struct TrackRecord {
    pub id: i64,
    pub folder_id: i64,
    pub path: String,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub album_artist: Option<String>,
    pub genre: Option<String>,
    pub year: Option<i32>,
    pub track_number: Option<i32>,
    pub disc_number: Option<i32>,
    pub duration_ms: i64,
    pub sample_rate: u32,
    pub bit_depth: u32,
    pub format: String,
    pub file_size: u64,
    pub modified_time: i64,
    pub thumbnail_path: Option<String>,
    pub has_lyrics: bool,
    pub play_count: u64,
    pub last_played_at: Option<i64>,
    pub is_active: bool,
}

#[derive(Debug, Clone)]
pub struct LyricLine {
    pub timestamp_ms: i64,
    pub text: String,
}

#[derive(Debug, Clone)]
pub struct LyricsRecord {
    pub track_id: i64,
    pub is_synced: bool,
    pub raw_text: String,
    pub lines: Vec<LyricLine>,
}

#[derive(Debug, Clone)]
pub struct Playlist {
    pub id: i64,
    pub name: String,
    pub description: Option<String>,
    pub custom_artwork_path: Option<String>,
    pub created_at: i64,
    pub updated_at: i64,
    pub track_count: usize,
}

#[derive(Debug, Clone)]
pub struct PlaylistTrackItem {
    pub id: i64,
    pub playlist_id: i64,
    pub track_id: i64,
    pub sort_order: i64,
    pub track: TrackRecord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortCriteria {
    Title,
    Artist,
    Album,
    TrackNumber,
    Duration,
    DateAdded,
    AudioSpecs,
}

impl Default for SortCriteria {
    fn default() -> Self {
        Self::Title
    }
}

impl SortCriteria {
    pub fn from_i32(val: i32) -> Self {
        match val {
            1 => Self::Artist,
            2 => Self::Album,
            3 => Self::TrackNumber,
            4 => Self::Duration,
            5 => Self::DateAdded,
            6 => Self::AudioSpecs,
            _ => Self::Title,
        }
    }

    pub fn to_i32(self) -> i32 {
        match self {
            Self::Title => 0,
            Self::Artist => 1,
            Self::Album => 2,
            Self::TrackNumber => 3,
            Self::Duration => 4,
            Self::DateAdded => 5,
            Self::AudioSpecs => 6,
        }
    }
}

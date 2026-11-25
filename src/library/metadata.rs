use std::fs;
use std::path::Path;
use lofty::file::AudioFile;
use lofty::prelude::*;
use lofty::probe::Probe;
use lofty::tag::ItemKey;

use crate::db::models::LyricLine;

#[derive(Debug, Clone)]
pub struct ParsedMetadata {
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
    pub cover_art: Option<Vec<u8>>,
    pub lyrics: Option<String>,
    pub is_synced_lyrics: bool,
}

pub fn extract_metadata(path: &Path) -> Result<ParsedMetadata, Box<dyn std::error::Error + Send + Sync>> {
    let metadata = fs::metadata(path)?;
    let file_size = metadata.len();
    let modified_time = metadata
        .modified()?
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);

    let tagged_file = Probe::open(path)?.read()?;
    let properties = tagged_file.properties();

    let duration_ms = properties.duration().as_millis() as i64;
    let sample_rate = properties.sample_rate().unwrap_or(44100);
    let bit_depth = properties.bit_depth().unwrap_or(16) as u32;

    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_uppercase();

    let tag = tagged_file.primary_tag().or_else(|| tagged_file.first_tag());

    let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("Unknown Title");

    let mut title = file_stem.to_string();
    let mut artist = "Unknown Artist".to_string();
    let mut album = "Unknown Album".to_string();
    let mut album_artist = None;
    let mut genre = None;
    let mut year = None;
    let mut track_number = None;
    let mut disc_number = None;
    let mut cover_art = None;
    let mut lyrics = None;
    let mut is_synced_lyrics = false;

    if let Some(t) = tag {
        if let Some(val) = t.title() {
            if !val.trim().is_empty() {
                title = val.trim().to_string();
            }
        }
        if let Some(val) = t.artist() {
            if !val.trim().is_empty() {
                artist = val.trim().to_string();
            }
        }
        if let Some(val) = t.album() {
            if !val.trim().is_empty() {
                album = val.trim().to_string();
            }
        }
        if let Some(item) = t.get(ItemKey::AlbumArtist) {
            if let Some(val) = item.value().text() {
                album_artist = Some(val.trim().to_string());
            }
        }
        if let Some(val) = t.genre() {
            genre = Some(val.trim().to_string());
        }
        if let Some(item) = t.get(ItemKey::Year).or_else(|| t.get(ItemKey::RecordingDate)) {
            if let Some(txt) = item.value().text() {
                let first_4: String = txt.chars().take(4).collect();
                if let Ok(y) = first_4.parse::<i32>() {
                    year = Some(y);
                }
            }
        }
        if let Some(val) = t.track() {
            track_number = Some(val as i32);
        }
        if let Some(val) = t.disk() {
            disc_number = Some(val as i32);
        }

        // Cover artwork: prefer front cover, else first picture
        for pic in t.pictures() {
            if pic.pic_type() == lofty::picture::PictureType::CoverFront {
                cover_art = Some(pic.data().to_vec());
                break;
            }
        }
        if cover_art.is_none() && !t.pictures().is_empty() {
            cover_art = Some(t.pictures()[0].data().to_vec());
        }

        // Embedded Lyrics extraction from primary tag (check Lyrics, UnsyncLyrics, and Unknown lyric keys)
        if let Some(found) = extract_lyrics_from_tag(t) {
            is_synced_lyrics = check_is_synced(&found);
            lyrics = Some(found);
        }
    }

    // Fallback: check ALL tags on the file if primary tag had no lyrics (e.g. FLAC with both ID3v2 and Vorbis)
    if lyrics.is_none() {
        for t in tagged_file.tags() {
            if let Some(found) = extract_lyrics_from_tag(t) {
                is_synced_lyrics = check_is_synced(&found);
                lyrics = Some(found);
                break;
            }
        }
    }

    // Fallback: external .lrc file in the same directory
    if lyrics.is_none() {
        let lrc_path = path.with_extension("lrc");
        if lrc_path.exists() && lrc_path.is_file() {
            if let Ok(content) = fs::read_to_string(&lrc_path) {
                let trimmed = content.trim();
                if !trimmed.is_empty() {
                    lyrics = Some(trimmed.to_string());
                    is_synced_lyrics = check_is_synced(trimmed);
                }
            }
        }
    }

    Ok(ParsedMetadata {
        title,
        artist,
        album,
        album_artist,
        genre,
        year,
        track_number,
        disc_number,
        duration_ms,
        sample_rate,
        bit_depth,
        format: ext,
        file_size,
        modified_time,
        cover_art,
        lyrics,
        is_synced_lyrics,
    })
}

fn extract_lyrics_from_tag(tag: &lofty::tag::Tag) -> Option<String> {
    // 1. Standard ItemKey::Lyrics (Vorbis LYRICS, MP4 ©lyr, APE Lyrics)
    for item in tag.get_items(ItemKey::Lyrics) {
        if let Some(text) = item.value().text() {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }

    // 2. ItemKey::UnsyncLyrics (ID3v2 USLT, Vorbis UNSYNCEDLYRICS, MP4 ©lyr)
    for item in tag.get_items(ItemKey::UnsyncLyrics) {
        if let Some(text) = item.value().text() {
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                return Some(trimmed.to_string());
            }
        }
    }

    None
}

pub fn check_is_synced(raw: &str) -> bool {
    let normalized = normalize_inline_lrc(raw);
    normalized.lines().any(|line| {
        let trimmed = line.trim();
        if let (Some(open), Some(close)) = (trimmed.find('['), trimmed.find(']')) {
            if open == 0 && close > 1 {
                return parse_timestamp_ms(&trimmed[1..close]).is_some();
            }
        }
        false
    })
}

fn normalize_inline_lrc(raw: &str) -> String {
    // If the entire lyrics tag was stored on a single line (no \n or \r) with multiple [mm:ss...] tags,
    // split before each timestamp bracket so every line parses cleanly.
    let mut result = raw.replace('\r', "\n");
    if result.lines().count() <= 1 && result.matches('[').count() > 1 {
        let mut out = String::with_capacity(result.len() + 32);
        let bytes = result.as_bytes();
        let mut i = 0;
        while i < bytes.len() {
            if bytes[i] == b'[' && i > 0 {
                // Check if this looks like [dd:dd...]
                let rest = &result[i..];
                if let Some(close) = rest.find(']') {
                    if parse_timestamp_ms(&rest[1..close]).is_some() {
                        out.push('\n');
                    }
                }
            }
            let ch = result[i..].chars().next().unwrap();
            out.push(ch);
            i += ch.len_utf8();
        }
        result = out;
    }
    result
}

pub fn parse_lrc_lines(raw: &str) -> Vec<LyricLine> {
    let normalized = normalize_inline_lrc(raw);
    let mut lines = Vec::new();

    for line in normalized.lines() {
        let trimmed = line.trim();
        if !trimmed.starts_with('[') {
            continue;
        }

        let mut remaining = trimmed;
        let mut timestamps = Vec::new();

        while remaining.starts_with('[') {
            if let Some(close_idx) = remaining.find(']') {
                let tag = &remaining[1..close_idx];
                if let Some(ms) = parse_timestamp_ms(tag) {
                    timestamps.push(ms);
                }
                remaining = &remaining[close_idx + 1..];
            } else {
                break;
            }
        }

        let lyric_text = remaining.trim().to_string();
        for ts in timestamps {
            lines.push(LyricLine {
                timestamp_ms: ts,
                text: lyric_text.clone(),
            });
        }
    }

    lines.sort_by_key(|l| l.timestamp_ms);
    lines
}

fn parse_timestamp_ms(tag: &str) -> Option<i64> {
    let parts: Vec<&str> = tag.split(':').collect();
    if parts.len() == 2 {
        let minutes: i64 = parts[0].trim().parse().ok()?;
        let sec_parts: Vec<&str> = parts[1].split('.').collect();
        if sec_parts.is_empty() {
            return None;
        }

        let seconds: i64 = sec_parts[0].trim().parse().ok()?;
        let ms: i64 = if sec_parts.len() > 1 {
            parse_fraction_ms(sec_parts[1].trim())?
        } else {
            0
        };

        Some(minutes * 60_000 + seconds * 1000 + ms)
    } else if parts.len() == 3 {
        // Support [mm:ss:xx] format (e.g. [00:00:07])
        let minutes: i64 = parts[0].trim().parse().ok()?;
        let seconds: i64 = parts[1].trim().parse().ok()?;
        let ms: i64 = parse_fraction_ms(parts[2].trim())?;
        Some(minutes * 60_000 + seconds * 1000 + ms)
    } else {
        None
    }
}

fn parse_fraction_ms(frac: &str) -> Option<i64> {
    if frac.is_empty() {
        return Some(0);
    }
    if frac.len() == 2 {
        Some(frac.parse::<i64>().ok()? * 10)
    } else if frac.len() == 3 {
        Some(frac.parse::<i64>().ok()?)
    } else if frac.len() == 1 {
        Some(frac.parse::<i64>().ok()? * 100)
    } else {
        Some(0)
    }
}

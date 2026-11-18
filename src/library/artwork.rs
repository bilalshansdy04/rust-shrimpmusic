use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use directories::ProjectDirs;
use image::imageops::FilterType;
use image::ImageReader;
use sha2::{Digest, Sha256};

pub fn get_thumbnail_dir() -> PathBuf {
    if let Some(proj_dirs) = ProjectDirs::from("com", "ShrimpMusic", "ShrimpMusic") {
        let dir = proj_dirs.data_local_dir().join("thumbnails");
        let _ = fs::create_dir_all(&dir);
        dir
    } else {
        let dir = PathBuf::from("thumbnails");
        let _ = fs::create_dir_all(&dir);
        dir
    }
}

pub fn get_playlist_artwork_dir() -> PathBuf {
    if let Some(proj_dirs) = ProjectDirs::from("com", "ShrimpMusic", "ShrimpMusic") {
        let dir = proj_dirs.data_local_dir().join("playlists");
        let _ = fs::create_dir_all(&dir);
        dir
    } else {
        let dir = PathBuf::from("playlists");
        let _ = fs::create_dir_all(&dir);
        dir
    }
}

pub fn generate_and_save_thumbnail(image_data: &[u8], key: &str) -> Option<String> {
    if image_data.is_empty() {
        return None;
    }

    let mut hasher = Sha256::new();
    hasher.update(key.as_bytes());
    let hash: String = hasher
        .finalize()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect();

    let thumb_dir = get_thumbnail_dir();
    let thumb_path = thumb_dir.join(format!("{}.jpg", hash));

    if thumb_path.exists() {
        return Some(thumb_path.to_string_lossy().to_string());
    }

    // Load and downscale using Lanczos3 to strictly 250x250px
    let reader = ImageReader::new(Cursor::new(image_data))
        .with_guessed_format()
        .ok()?;
    let img = reader.decode().ok()?;

    let resized = img.resize_to_fill(250, 250, FilterType::Lanczos3);
    resized.save_with_format(&thumb_path, image::ImageFormat::Jpeg).ok()?;

    Some(thumb_path.to_string_lossy().to_string())
}

pub fn save_custom_playlist_artwork(image_path: &Path, playlist_id: i64) -> Option<String> {
    let playlist_dir = get_playlist_artwork_dir();
    let target_path = playlist_dir.join(format!("{}.jpg", playlist_id));

    let img = ImageReader::open(image_path).ok()?.decode().ok()?;
    let resized = img.resize_to_fill(250, 250, FilterType::Lanczos3);
    resized.save_with_format(&target_path, image::ImageFormat::Jpeg).ok()?;

    Some(target_path.to_string_lossy().to_string())
}

pub fn clear_thumbnail_cache() -> std::io::Result<usize> {
    let dir = get_thumbnail_dir();
    let mut count = 0;
    if dir.exists() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            if entry.path().is_file() {
                if fs::remove_file(entry.path()).is_ok() {
                    count += 1;
                }
            }
        }
    }
    Ok(count)
}

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use directories::ProjectDirs;
use fast_image_resize::images::Image;
use fast_image_resize::{FilterType, PixelType, ResizeAlg, Resizer};
use image::{DynamicImage, ImageReader};
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

/// Downscale image to target square dimensions (e.g. 500x500) using fast_image_resize SIMD Lanczos3.
/// Prevents aliasing, blurring, and pixelated borders while keeping RAM < 10 MB.
pub fn resize_image_lanczos3(img: &DynamicImage, target_w: u32, target_h: u32) -> Option<DynamicImage> {
    let width = img.width();
    let height = img.height();

    // 1. Crop to square (aspect fill) before downscaling
    let min_dim = width.min(height);
    let crop_x = (width - min_dim) / 2;
    let crop_y = (height - min_dim) / 2;
    let cropped = img.crop_imm(crop_x, crop_y, min_dim, min_dim);

    let src_rgba = cropped.to_rgba8();
    let (src_w, src_h) = src_rgba.dimensions();

    let src_image = Image::from_vec_u8(
        src_w,
        src_h,
        src_rgba.into_raw(),
        PixelType::U8x4,
    ).ok()?;

    let mut dst_image = Image::new(target_w, target_h, PixelType::U8x4);

    let mut resizer = Resizer::new();
    // Use Lanczos3 filter for superior anti-aliasing and sharpness
    resizer.resize(
        &src_image,
        &mut dst_image,
        &fast_image_resize::ResizeOptions::new().resize_alg(ResizeAlg::Interpolation(FilterType::Lanczos3)),
    ).ok()?;

    let dst_buffer = dst_image.into_vec();
    let rgba_img = image::RgbaImage::from_raw(target_w, target_h, dst_buffer)?;
    Some(DynamicImage::ImageRgba8(rgba_img))
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

    // Load and downscale using SIMD Lanczos3 Anti-Aliased filter to 500x500px (retina-sharp on Windows High-DPI)
    let reader = ImageReader::new(Cursor::new(image_data))
        .with_guessed_format()
        .ok()?;
    let img = reader.decode().ok()?;

    let resized = resize_image_lanczos3(&img, 500, 500)?;
    resized.save_with_format(&thumb_path, image::ImageFormat::Jpeg).ok()?;

    Some(thumb_path.to_string_lossy().to_string())
}

pub fn save_custom_playlist_artwork(image_path: &Path, playlist_id: i64) -> Option<String> {
    let playlist_dir = get_playlist_artwork_dir();
    let target_path = playlist_dir.join(format!("{}.jpg", playlist_id));

    let img = ImageReader::open(image_path).ok()?.decode().ok()?;
    let resized = resize_image_lanczos3(&img, 500, 500)?;
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

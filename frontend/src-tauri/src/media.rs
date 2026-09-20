use crate::scanner::is_video_ext;
use image::imageops::FilterType;
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::io::Cursor;
use std::path::Path;
use std::sync::Mutex;

const MAX_THUMB_CACHE: usize = 500;

struct CacheState {
    map: HashMap<String, (Vec<u8>, String)>,
    order: VecDeque<String>,
}

static THUMB_CACHE: Mutex<Option<CacheState>> = Mutex::new(None);

pub fn get_cached_thumb(key: &str) -> Option<(Vec<u8>, String)> {
    let mut lock = THUMB_CACHE.lock().unwrap();
    if lock.is_none() {
        *lock = Some(CacheState {
            map: HashMap::new(),
            order: VecDeque::new(),
        });
    }
    let state = lock.as_mut().unwrap();
    state.map.get(key).cloned()
}

pub fn set_cached_thumb(key: String, content: Vec<u8>, mime: String) {
    let mut lock = THUMB_CACHE.lock().unwrap();
    if lock.is_none() {
        *lock = Some(CacheState {
            map: HashMap::new(),
            order: VecDeque::new(),
        });
    }
    let state = lock.as_mut().unwrap();

    if state.map.len() >= MAX_THUMB_CACHE {
        if let Some(oldest) = state.order.pop_front() {
            state.map.remove(&oldest);
        }
    }

    state.order.push_back(key.clone());
    state.map.insert(key, (content, mime));
}

pub fn generate_video_thumbnail_svg(filename: &str, max_dim: u32) -> Vec<u8> {
    let escaped = filename
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;");

    let display_name = if escaped.len() > 28 {
        format!("{}...", &escaped[..25])
    } else {
        escaped
    };

    let width = max_dim;
    let height = (max_dim as f32 * 0.65) as u32;
    let cx = width / 2;
    let cy = height / 2 - 12;

    let svg = format!(
        r##"<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">
  <defs>
    <linearGradient id="dcVideoGrad" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#0f172a" />
      <stop offset="50%" stop-color="#1e293b" />
      <stop offset="100%" stop-color="#090d16" />
    </linearGradient>
  </defs>
  <rect width="100%" height="100%" fill="url(#dcVideoGrad)" rx="8" />
  <circle cx="16" cy="14" r="3" fill="#475569" />
  <circle cx="28" cy="14" r="3" fill="#475569" />
  <circle cx="40" cy="14" r="3" fill="#475569" />
  <circle cx="{cx}" cy="{cy}" r="28" fill="#38bdf8" opacity="0.95" />
  <polygon points="{p1_x},{p1_y} {p2_x},{p2_y} {p3_x},{p3_y}" fill="#0f172a" />
  <rect x="{badge_x}" y="{badge_y}" width="64" height="20" rx="4" fill="#334155" />
  <text x="{cx}" y="{badge_text_y}" fill="#f8fafc" font-size="11" font-weight="700" font-family="system-ui, -apple-system, sans-serif" text-anchor="middle">VIDEO</text>
  <text x="{cx}" y="{name_y}" fill="#94a3b8" font-size="12" font-family="system-ui, -apple-system, sans-serif" text-anchor="middle">{display_name}</text>
</svg>"##,
        width = width,
        height = height,
        cx = cx,
        cy = cy,
        p1_x = cx.saturating_sub(7),
        p1_y = cy.saturating_sub(12),
        p2_x = cx + 13,
        p2_y = cy,
        p3_x = cx.saturating_sub(7),
        p3_y = cy + 12,
        badge_x = cx.saturating_sub(32),
        badge_y = cy + 36,
        badge_text_y = cy + 50,
        name_y = height.saturating_sub(14),
        display_name = display_name
    );

    svg.into_bytes()
}

pub fn decode_heic(bytes: &[u8]) -> Result<image::DynamicImage, String> {
    let output = heic::DecoderConfig::new()
        .decode(bytes, heic::PixelLayout::Rgba8)
        .map_err(|e| format!("Failed to decode HEIC: {:?}", e))?;
    let img_buf = image::RgbaImage::from_raw(output.width as u32, output.height as u32, output.data)
        .ok_or_else(|| "Failed to construct RGBA image from HEIC buffer".to_string())?;
    Ok(image::DynamicImage::ImageRgba8(img_buf))
}

pub fn to_rgb_with_background(img: &image::DynamicImage) -> image::RgbImage {
    match img {
        image::DynamicImage::ImageRgba8(rgba) => {
            let mut rgb = image::RgbImage::new(rgba.width(), rgba.height());
            for (x, y, pixel) in rgba.enumerate_pixels() {
                let [r, g, b, a] = pixel.0;
                if a == 255 {
                    rgb.put_pixel(x, y, image::Rgb([r, g, b]));
                } else if a == 0 {
                    // Transparent pixels blended onto theme dark neutral background #18181b (24, 24, 27)
                    rgb.put_pixel(x, y, image::Rgb([24, 24, 27]));
                } else {
                    let alpha = a as f32 / 255.0;
                    let bg_r = 24.0;
                    let bg_g = 24.0;
                    let bg_b = 27.0;
                    let comp_r = (r as f32 * alpha + bg_r * (1.0 - alpha)).round() as u8;
                    let comp_g = (g as f32 * alpha + bg_g * (1.0 - alpha)).round() as u8;
                    let comp_b = (b as f32 * alpha + bg_b * (1.0 - alpha)).round() as u8;
                    rgb.put_pixel(x, y, image::Rgb([comp_r, comp_g, comp_b]));
                }
            }
            rgb
        }
        _ => img.to_rgb8(),
    }
}

fn extract_video_thumbnail(filepath: &Path, max_dim: u32) -> Option<Vec<u8>> {
    let scale = format!(
        "scale='if(gt(iw,ih),min(iw,{}),-2)':'if(gt(ih,iw),min(ih,{}),-2)'",
        max_dim, max_dim
    );
    let output = std::process::Command::new("ffmpeg")
        .args([
            "-hide_banner", "-loglevel", "error", "-i",
        ])
        .arg(filepath)
        .args([
            "-frames:v", "1", "-vf",
        ])
        .arg(scale)
        .args(["-f", "image2pipe", "-vcodec", "mjpeg", "-q:v", "4", "pipe:1"])
        .output()
        .ok()?;
    if !output.status.success() || output.stdout.is_empty() {
        return None;
    }

    let image = image::load_from_memory(&output.stdout).ok()?;
    let resized = if image.width() > max_dim || image.height() > max_dim {
        let scale = max_dim as f32 / image.width().max(image.height()) as f32;
        image.resize(
            (image.width() as f32 * scale).round() as u32,
            (image.height() as f32 * scale).round() as u32,
            FilterType::Lanczos3,
        )
    } else {
        image
    };
    let mut buf = Cursor::new(Vec::new());
    to_rgb_with_background(&resized)
        .write_to(&mut buf, image::ImageFormat::Jpeg)
        .ok()?;
    Some(buf.into_inner())
}

pub fn render_thumbnail(filepath: &Path, max_dim: u32) -> Result<(Vec<u8>, String), String> {
    let path_str = filepath.to_string_lossy().to_string();
    let cache_key = format!("{}_{}", path_str, max_dim);

    if let Some(cached) = get_cached_thumb(&cache_key) {
        return Ok(cached);
    }

    let ext = filepath.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if is_video_ext(&ext) {
        if let Some(frame) = extract_video_thumbnail(filepath, max_dim) {
            let mime = "image/jpeg".to_string();
            set_cached_thumb(cache_key, frame.clone(), mime.clone());
            return Ok((frame, mime));
        }
        let filename = filepath.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let svg = generate_video_thumbnail_svg(filename, max_dim);
        let mime = "image/svg+xml".to_string();
        set_cached_thumb(cache_key, svg.clone(), mime.clone());
        return Ok((svg, mime));
    }

    let img = if ext == "heic" || ext == "heif" {
        let bytes = fs::read(filepath).map_err(|e| format!("Failed to read HEIC file: {}", e))?;
        decode_heic(&bytes)?
    } else {
        image::open(filepath).map_err(|e| format!("Failed to open image: {}", e))?
    };

    let (w, h) = (img.width(), img.height());

    let resized = if w > max_dim || h > max_dim {
        let scale = (max_dim as f32) / (w.max(h) as f32);
        let new_w = (w as f32 * scale).round() as u32;
        let new_h = (h as f32 * scale).round() as u32;
        img.resize(new_w, new_h, FilterType::Lanczos3)
    } else {
        img
    };

    let rgb_img = to_rgb_with_background(&resized);

    let mut buf = Cursor::new(Vec::new());
    rgb_img
        .write_to(&mut buf, image::ImageFormat::Jpeg)
        .map_err(|e| format!("Failed to encode JPEG thumbnail: {}", e))?;

    let bytes = buf.into_inner();
    let mime = "image/jpeg".to_string();
    set_cached_thumb(cache_key, bytes.clone(), mime.clone());
    Ok((bytes, mime))
}

pub fn read_full_media(filepath: &Path) -> Result<(Vec<u8>, String), String> {
    let ext = filepath.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if ext == "heic" || ext == "heif" {
        let bytes = fs::read(filepath).map_err(|e| format!("Failed to read HEIC file: {}", e))?;
        let img = decode_heic(&bytes)?;
        let rgb_img = to_rgb_with_background(&img);
        let mut buf = Cursor::new(Vec::new());
        rgb_img
            .write_to(&mut buf, image::ImageFormat::Jpeg)
            .map_err(|e| format!("Failed to encode full HEIC as JPEG: {}", e))?;
        return Ok((buf.into_inner(), "image/jpeg".to_string()));
    }

    let bytes = fs::read(filepath).map_err(|e| format!("Failed to read media: {}", e))?;
    let mime = match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "avi" => "video/x-msvideo",
        "m4v" => "video/x-m4v",
        "mkv" => "video/x-matroska",
        _ => "application/octet-stream",
    };
    Ok((bytes, mime.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_png_rgba_thumbnail_rendering() {
        let temp_dir = std::env::temp_dir();
        let png_path = temp_dir.join(format!(
            "test_thumb_{}.png",
            std::time::SystemTime::now().elapsed().unwrap().as_nanos()
        ));

        // Create a 50x50 transparent PNG
        let mut rgba = image::RgbaImage::new(50, 50);
        for (x, _y, p) in rgba.enumerate_pixels_mut() {
            if x < 25 {
                *p = image::Rgba([255, 0, 0, 128]); // Semi-transparent red
            } else {
                *p = image::Rgba([0, 255, 0, 255]); // Opaque green
            }
        }
        rgba.save(&png_path).expect("Failed to save test PNG");

        let result = render_thumbnail(&png_path, 32);
        let _ = fs::remove_file(&png_path);

        assert!(
            result.is_ok(),
            "render_thumbnail failed for RGBA PNG: {:?}",
            result.err()
        );
        let (bytes, mime) = result.unwrap();
        assert_eq!(mime, "image/jpeg");
        assert!(!bytes.is_empty());

        let decoded =
            image::load_from_memory(&bytes).expect("Decoded thumbnail is not a valid JPEG");
        assert!(decoded.width() <= 32);
        assert!(decoded.height() <= 32);
    }

    #[test]
    fn test_heic_thumbnail_and_full_rendering() {
        let fixture_path = Path::new("tests/fixtures/sample.heic");
        let target_path = if fixture_path.exists() {
            fixture_path
        } else {
            Path::new("frontend/src-tauri/tests/fixtures/sample.heic")
        };

        if !target_path.exists() {
            return;
        }

        let result = render_thumbnail(target_path, 32);
        assert!(
            result.is_ok(),
            "render_thumbnail failed for HEIC: {:?}",
            result.err()
        );
        let (bytes, mime) = result.unwrap();
        assert_eq!(mime, "image/jpeg");
        assert!(!bytes.is_empty());

        let full_result = read_full_media(target_path);
        assert!(
            full_result.is_ok(),
            "read_full_media failed for HEIC: {:?}",
            full_result.err()
        );
        let (full_bytes, full_mime) = full_result.unwrap();
        assert_eq!(full_mime, "image/jpeg");
        assert!(!full_bytes.is_empty());
    }

    #[test]
    fn test_to_rgb_with_background_transparency() {
        let mut rgba = image::RgbaImage::new(2, 2);
        rgba.put_pixel(0, 0, image::Rgba([100, 150, 200, 255])); // Opaque
        rgba.put_pixel(1, 1, image::Rgba([0, 0, 0, 0])); // Fully transparent

        let dynamic = image::DynamicImage::ImageRgba8(rgba);
        let rgb = to_rgb_with_background(&dynamic);

        assert_eq!(rgb.get_pixel(0, 0).0, [100, 150, 200]);
        assert_eq!(rgb.get_pixel(1, 1).0, [24, 24, 27]); // Theme dark neutral
    }

    #[test]
    fn test_corrupt_video_uses_svg_fallback() {
        let video_path = std::env::temp_dir().join(format!(
            "dreamcatcher-corrupt-video-{}.mkv",
            std::process::id()
        ));
        fs::write(&video_path, b"not a real video").expect("Failed to write test video");

        let result = render_thumbnail(&video_path, 120).expect("Fallback should succeed");
        let _ = fs::remove_file(&video_path);

        assert_eq!(result.1, "image/svg+xml");
        assert!(String::from_utf8_lossy(&result.0).contains("VIDEO"));
    }

    #[test]
    fn test_video_frame_extraction_preserves_max_dim() {
        if std::process::Command::new("ffmpeg")
            .arg("-version")
            .output()
            .is_err()
        {
            return;
        }

        let video_path = std::env::temp_dir().join(format!(
            "dreamcatcher-video-{}.mp4",
            std::process::id()
        ));
        let created = std::process::Command::new("ffmpeg")
            .args([
                "-hide_banner", "-loglevel", "error", "-f", "lavfi", "-i",
                "color=c=blue:s=640x360:d=1", "-frames:v", "1",
            ])
            .arg(&video_path)
            .status()
            .expect("Failed to invoke ffmpeg");
        if !created.success() {
            return;
        }

        let result = render_thumbnail(&video_path, 120).expect("Frame extraction should succeed");
        let _ = fs::remove_file(&video_path);
        if result.1 != "image/jpeg" {
            return;
        }
        let decoded = image::load_from_memory(&result.0).expect("Frame should be a valid JPEG");

        assert_eq!(result.1, "image/jpeg");
        assert_eq!((decoded.width(), decoded.height()), (120, 68));
    }
}

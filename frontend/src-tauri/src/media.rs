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

pub fn render_thumbnail(filepath: &Path, max_dim: u32) -> Result<(Vec<u8>, String), String> {
    let path_str = filepath.to_string_lossy().to_string();
    let cache_key = format!("{}_{}", path_str, max_dim);

    if let Some(cached) = get_cached_thumb(&cache_key) {
        return Ok(cached);
    }

    let ext = filepath.extension().and_then(|e| e.to_str()).unwrap_or("");
    if is_video_ext(ext) {
        let filename = filepath.file_name().and_then(|n| n.to_str()).unwrap_or("");
        let svg = generate_video_thumbnail_svg(filename, max_dim);
        let mime = "image/svg+xml".to_string();
        set_cached_thumb(cache_key, svg.clone(), mime.clone());
        return Ok((svg, mime));
    }

    let img = image::open(filepath).map_err(|e| format!("Failed to open image: {}", e))?;
    let (w, h) = (img.width(), img.height());

    let resized = if w > max_dim || h > max_dim {
        let scale = (max_dim as f32) / (w.max(h) as f32);
        let new_w = (w as f32 * scale).round() as u32;
        let new_h = (h as f32 * scale).round() as u32;
        img.resize(new_w, new_h, FilterType::Lanczos3)
    } else {
        img
    };

    let mut buf = Cursor::new(Vec::new());
    resized
        .write_to(&mut buf, image::ImageFormat::Jpeg)
        .map_err(|e| format!("Failed to encode JPEG thumbnail: {}", e))?;

    let bytes = buf.into_inner();
    let mime = "image/jpeg".to_string();
    set_cached_thumb(cache_key, bytes.clone(), mime.clone());
    Ok((bytes, mime))
}

pub fn read_full_media(filepath: &Path) -> Result<(Vec<u8>, String), String> {
    let bytes = fs::read(filepath).map_err(|e| format!("Failed to read media: {}", e))?;
    let ext = filepath.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let mime = match ext.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "gif" => "image/gif",
        "mp4" => "video/mp4",
        "mov" => "video/quicktime",
        "mkv" => "video/x-matroska",
        _ => "application/octet-stream",
    };
    Ok((bytes, mime.to_string()))
}

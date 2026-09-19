use crate::scanner::is_video_ext;
use base64::prelude::*;
use image::imageops::FilterType;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisionCacheEntry {
    pub category: String,
    pub tier: String,
    pub reason: String,
    pub details: String,
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationResult {
    pub category: String,
    pub tier: String,
    pub reason: String,
    pub caption: String,
    pub is_cached: bool,
}

// Thread-safe in-memory cache wrapped in Mutex
static CACHE: Mutex<Option<HashMap<String, VisionCacheEntry>>> = Mutex::new(None);

fn get_cache_file_path() -> PathBuf {
    PathBuf::from("vision_cache.json")
}

fn load_cache() -> HashMap<String, VisionCacheEntry> {
    let path = get_cache_file_path();
    if path.exists() {
        if let Ok(content) = fs::read_to_string(&path) {
            if let Ok(map) = serde_json::from_str::<HashMap<String, VisionCacheEntry>>(&content) {
                return map;
            }
        }
    }
    HashMap::new()
}

fn save_cache(cache: &HashMap<String, VisionCacheEntry>) {
    let path = get_cache_file_path();
    let temp = path.with_extension("tmp");
    if let Ok(json_str) = serde_json::to_string_pretty(cache) {
        if fs::write(&temp, json_str).is_ok() {
            let _ = fs::rename(&temp, &path);
        }
    }
}

pub fn lookup_cache(filepath: &Path) -> Option<VisionCacheEntry> {
    let name = filepath.file_name()?.to_str()?;
    let size = filepath.metadata().map(|m| m.len()).unwrap_or(0);

    let mut lock = CACHE.lock().unwrap();
    if lock.is_none() {
        *lock = Some(load_cache());
    }
    let map = lock.as_ref().unwrap();

    let keys = [
        format!("dc_{}_{}", name, size),
        format!("doc_check_{}_{}", name, size),
        format!("doc_check_{}", name),
    ];

    for k in &keys {
        if let Some(entry) = map.get(k) {
            return Some(entry.clone());
        }
    }
    None
}

pub fn put_cache(filepath: &Path, entry: VisionCacheEntry) {
    let name = match filepath.file_name().and_then(|n| n.to_str()) {
        Some(n) => n,
        None => return,
    };
    let size = filepath.metadata().map(|m| m.len()).unwrap_or(0);
    let key = format!("dc_{}_{}", name, size);

    let mut lock = CACHE.lock().unwrap();
    if lock.is_none() {
        *lock = Some(load_cache());
    }
    if let Some(map) = lock.as_mut() {
        map.insert(key, entry);
        save_cache(map);
    }
}

pub fn get_downscaled_image_bytes(filepath: &Path, max_dim: u32) -> Option<Vec<u8>> {
    let ext = filepath.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    let img = if ext == "heic" || ext == "heif" {
        let bytes = fs::read(filepath).ok()?;
        crate::media::decode_heic(&bytes).ok()?
    } else {
        image::open(filepath).ok()?
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

    let rgb_img = crate::media::to_rgb_with_background(&resized);
    let mut buf = Cursor::new(Vec::new());
    rgb_img
        .write_to(&mut buf, image::ImageFormat::Jpeg)
        .ok()?;
    Some(buf.into_inner())
}

pub fn classify_heuristic(filepath: &Path) -> ClassificationResult {
    // 0. Cache check
    if let Some(cached) = lookup_cache(filepath) {
        return ClassificationResult {
            category: cached.category,
            tier: if cached.confidence > 0.85 {
                "OBVIOUS".to_string()
            } else {
                "MIXED".to_string()
            },
            reason: cached.reason,
            caption: cached.details,
            is_cached: true,
        };
    }

    let name = filepath.file_name().and_then(|n| n.to_str()).unwrap_or("").to_lowercase();
    let ext = filepath.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();

    // 1. Videos are obvious photos/memories
    if is_video_ext(&ext) {
        return ClassificationResult {
            category: "PHOTO".to_string(),
            tier: "OBVIOUS".to_string(),
            reason: "Video file".to_string(),
            caption: "Video recording".to_string(),
            is_cached: false,
        };
    }

    // 2. Filename patterns
    let doc_patterns = [
        "screenshot", "scan", "receipt", "invoice", "bill", "statement",
        "ticket", "label", "whiteboard", "note", "document", "card",
        "manual", "serial", "rx", "prescription", "wp-",
    ];

    for pat in &doc_patterns {
        if name.contains(pat) {
            return ClassificationResult {
                category: "DOCUMENT".to_string(),
                tier: "OBVIOUS".to_string(),
                reason: format!("Filename keyword: {}", pat),
                caption: String::new(),
                is_cached: false,
            };
        }
    }

    // 3. Aspect ratio check
    if let Ok(reader) = image::ImageReader::open(filepath) {
        if let Ok(dimensions) = reader.into_dimensions() {
            let (w, h) = dimensions;
            let max_side = w.max(h) as f32;
            let min_side = w.min(h).max(1) as f32;
            let aspect = max_side / min_side;
            if aspect > 2.5 {
                return ClassificationResult {
                    category: "DOCUMENT".to_string(),
                    tier: "OBVIOUS".to_string(),
                    reason: format!("Extreme scrolling aspect ratio ({:.2})", aspect),
                    caption: String::new(),
                    is_cached: false,
                };
            }
        }
    }

    // 4. Mixed / Ambiguous fallback
    ClassificationResult {
        category: "PHOTO".to_string(),
        tier: "MIXED".to_string(),
        reason: "Ambiguous (Awaiting Vision / Manual Triage)".to_string(),
        caption: String::new(),
        is_cached: false,
    }
}

pub async fn classify_with_moondream(
    client: &reqwest::Client,
    image_bytes: &[u8],
    ollama_model: &str,
    ollama_url: &str,
) -> Result<(String, String, String), String> {
    let b64_data = BASE64_STANDARD.encode(image_bytes);
    let payload = serde_json::json!({
        "model": ollama_model,
        "prompt": "Describe what is in this image in one short sentence.",
        "images": [b64_data],
        "stream": false
    });

    let resp = client
        .post(format!("{}/api/generate", ollama_url))
        .json(&payload)
        .send()
        .await
        .map_err(|e| format!("Ollama request failed: {}", e))?;

    let json_resp: serde_json::Value = resp
        .json()
        .await
        .map_err(|e| format!("Failed to parse Ollama response: {}", e))?;

    let caption = json_resp
        .get("response")
        .and_then(|r| r.as_str())
        .unwrap_or("")
        .trim()
        .to_string();

    let lower = caption.to_lowercase();

    let doc_indicators = [
        ("receipt", "Receipt or store purchase"),
        ("invoice", "Invoice detected"),
        ("bill", "Billing statement"),
        ("document", "Document or paperwork"),
        ("paperwork", "Paperwork detected"),
        ("screen", "Computer or display screen"),
        ("monitor", "Computer monitor"),
        ("laptop", "Laptop screen"),
        ("screenshot", "Screenshot capture"),
        ("barcode", "Barcode or QR code tag"),
        ("label", "Product/equipment label"),
        ("serial number", "Serial number plate"),
        ("prescription", "Medical prescription or medication"),
        ("whiteboard", "Whiteboard notes"),
        ("text on paper", "Dense printed text"),
    ];

    for (kw, reason) in &doc_indicators {
        if lower.contains(kw) {
            return Ok(("DOCUMENT".to_string(), reason.to_string(), caption));
        }
    }

    Ok(("PHOTO".to_string(), "Authentic visual scene".to_string(), caption))
}

pub async fn analyze_image(
    client: &reqwest::Client,
    filepath: &Path,
    backend: &str,
    ollama_model: &str,
    ollama_url: &str,
) -> ClassificationResult {
    let fast_res = classify_heuristic(filepath);
    if fast_res.tier == "OBVIOUS" {
        return fast_res;
    }

    let data = match get_downscaled_image_bytes(filepath, 384) {
        Some(d) => d,
        None => {
            return ClassificationResult {
                category: "PHOTO".to_string(),
                tier: "MIXED".to_string(),
                reason: "Unable to decode image bytes".to_string(),
                caption: String::new(),
                is_cached: false,
            };
        }
    };

    if backend == "ollama" {
        match classify_with_moondream(client, &data, ollama_model, ollama_url).await {
            Ok((cat, reason, caption)) => {
                let tier = if cat == "DOCUMENT" { "OBVIOUS" } else { "MIXED" };
                let entry = VisionCacheEntry {
                    category: cat.clone(),
                    tier: tier.to_string(),
                    reason: reason.clone(),
                    details: caption.clone(),
                    confidence: if cat == "DOCUMENT" { 0.95 } else { 0.8 },
                };
                put_cache(filepath, entry);

                ClassificationResult {
                    category: cat,
                    tier: tier.to_string(),
                    reason,
                    caption,
                    is_cached: false,
                }
            }
            Err(err) => ClassificationResult {
                category: "PHOTO".to_string(),
                tier: "MIXED".to_string(),
                reason: format!("Vision error: {}", err),
                caption: String::new(),
                is_cached: false,
            },
        }
    } else {
        // Fallback for cloud / offline
        fast_res
    }
}

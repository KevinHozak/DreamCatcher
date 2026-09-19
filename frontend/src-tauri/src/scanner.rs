use chrono::{DateTime, NaiveDate, NaiveDateTime, Utc};
use exif::{In, Reader, Tag, Value};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs::{self, File};
use std::io::BufReader;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaItem {
    pub id: String,
    pub name: String,
    pub path: String,
    pub ext: String,
    pub is_video: bool,
    pub size: u64,
    pub timestamp: String,
    pub date_str: String,
    pub month_str: String,
    pub is_undated: bool,
    pub has_sidecar: bool,
    pub sidecar_path: Option<String>,
    pub has_gps: bool,
    pub gps: Option<(f64, f64)>,
    pub category: String,
    pub tier: String,
    pub reason: String,
    pub caption: String,
    pub is_cached: Option<bool>,
}

pub const PHOTO_EXTS: &[&str] = &[
    "jpg", "jpeg", "png", "heic", "heif", "webp", "tiff", "tif", "gif",
];
pub const VIDEO_EXTS: &[&str] = &["mp4", "mov", "avi", "m4v", "mkv"];

pub fn is_video_ext(ext: &str) -> bool {
    let lower = ext.to_lowercase();
    VIDEO_EXTS.contains(&lower.as_str())
}

pub fn is_media_ext(ext: &str) -> bool {
    let lower = ext.to_lowercase();
    PHOTO_EXTS.contains(&lower.as_str()) || VIDEO_EXTS.contains(&lower.as_str())
}

pub fn find_json_sidecar(filepath: &Path) -> Option<PathBuf> {
    let p_dir = filepath.parent()?;
    let name = filepath.file_name()?.to_str()?;
    let stem = filepath.file_stem()?.to_str()?;
    let ext = filepath.extension().and_then(|e| e.to_str()).unwrap_or("");

    let mut candidates = vec![
        p_dir.join(format!("{}.supplemental-metadata.json", name)),
        p_dir.join(format!("{}.json", name)),
        p_dir.join(format!("{}.supplemental-metadata.json", stem)),
        p_dir.join(format!("{}.json", stem)),
    ];

    // Handle Google Takeout (1) variations: "IMG_001(1).jpg" -> "IMG_001.jpg(1).json"
    let re_dup = Regex::new(r"\((\d+)\)$").unwrap();
    if let Some(caps) = re_dup.captures(stem) {
        if let Some(m) = caps.get(1) {
            let dup_num = m.as_str();
            let base_stem = &stem[..caps.get(0).unwrap().start()];
            let dot_ext = if ext.is_empty() { String::new() } else { format!(".{}", ext) };
            candidates.push(p_dir.join(format!("{}{}({}).supplemental-metadata.json", base_stem, dot_ext, dup_num)));
            candidates.push(p_dir.join(format!("{}{}({}).json", base_stem, dot_ext, dup_num)));
            candidates.push(p_dir.join(format!("{}({}){}.supplemental-metadata.json", base_stem, dup_num, dot_ext)));
            candidates.push(p_dir.join(format!("{}({}){}.json", base_stem, dup_num, dot_ext)));
        }
    }

    for c in candidates {
        if c.exists() && c.is_file() {
            return Some(c);
        }
    }
    None
}

pub fn extract_timestamp(filepath: &Path, sidecar_path: Option<&Path>) -> (DateTime<Utc>, bool) {
    // 1. Sidecar metadata
    if let Some(sc) = sidecar_path {
        if sc.exists() {
            if let Ok(content) = fs::read_to_string(sc) {
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(ts_str) = data
                        .get("photoTakenTime")
                        .and_then(|p| p.get("timestamp"))
                        .and_then(|t| t.as_str())
                    {
                        if let Ok(ts_sec) = ts_str.parse::<i64>() {
                            if let Some(dt) = DateTime::from_timestamp(ts_sec, 0) {
                                return (dt, false);
                            }
                        }
                    }
                }
            }
        }
    }

    // 2. EXIF data
    let ext = filepath.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if PHOTO_EXTS.contains(&ext.as_str()) {
        if let Ok(file) = File::open(filepath) {
            let mut bufreader = BufReader::new(file);
            if let Ok(exif) = Reader::new().read_from_container(&mut bufreader) {
                for tag in &[Tag::DateTimeOriginal, Tag::DateTime] {
                    if let Some(field) = exif.get_field(*tag, In::PRIMARY) {
                        let s = field.display_value().to_string();
                        // EXIF date format: "YYYY:MM:DD HH:MM:SS"
                        let clean = s.trim().trim_matches('"');
                        if let Ok(naive) = NaiveDateTime::parse_from_str(clean, "%Y:%m:%d %H:%M:%S") {
                            return (DateTime::from_naive_utc_and_offset(naive, Utc), false);
                        }
                    }
                }
            }
        }
    }

    // 3. Filename patterns e.g. 20240512_143022 or 2024-05-12
    if let Some(name) = filepath.file_name().and_then(|n| n.to_str()) {
        let re = Regex::new(r"(20\d{2})[-_]?(\d{2})[-_]?(\d{2})").unwrap();
        if let Some(caps) = re.captures(name) {
            if let (Ok(y), Ok(m), Ok(d)) = (
                caps[1].parse::<i32>(),
                caps[2].parse::<u32>(),
                caps[3].parse::<u32>(),
            ) {
                if let Some(date) = NaiveDate::from_ymd_opt(y, m, d) {
                    if let Some(naive) = date.and_hms_opt(12, 0, 0) {
                        return (DateTime::from_naive_utc_and_offset(naive, Utc), false);
                    }
                }
            }
        }
    }

    // 4. Fallback: file mtime
    if let Ok(meta) = filepath.metadata() {
        if let Ok(modified) = meta.modified() {
            let dt: DateTime<Utc> = modified.into();
            return (dt, true);
        }
    }

    (Utc::now(), true)
}

fn parse_rational_deg(val: &Value, ref_str: Option<&str>) -> Option<f64> {
    match val {
        Value::Rational(rats) if rats.len() >= 3 => {
            let deg = rats[0].num as f64 / rats[0].denom.max(1) as f64;
            let min = rats[1].num as f64 / rats[1].denom.max(1) as f64;
            let sec = rats[2].num as f64 / rats[2].denom.max(1) as f64;
            let mut d = deg + min / 60.0 + sec / 3600.0;
            if let Some(r) = ref_str {
                let r_clean = r.trim().to_uppercase();
                if r_clean == "S" || r_clean == "W" {
                    d = -d;
                }
            }
            Some((d * 1000000.0).round() / 1000000.0)
        }
        _ => None,
    }
}

pub fn extract_gps_coordinates(filepath: &Path, sidecar_path: Option<&Path>) -> Option<(f64, f64)> {
    // 1. Takeout JSON sidecar
    if let Some(sc) = sidecar_path {
        if sc.exists() {
            if let Ok(content) = fs::read_to_string(sc) {
                if let Ok(data) = serde_json::from_str::<serde_json::Value>(&content) {
                    if let Some(geo) = data.get("geoData") {
                        let lat = geo.get("latitude").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        let lon = geo.get("longitude").and_then(|v| v.as_f64()).unwrap_or(0.0);
                        if lat != 0.0 || lon != 0.0 {
                            return Some((lat, lon));
                        }
                    }
                }
            }
        }
    }

    // 2. EXIF data
    let ext = filepath.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
    if PHOTO_EXTS.contains(&ext.as_str()) {
        if let Ok(file) = File::open(filepath) {
            let mut bufreader = BufReader::new(file);
            if let Ok(exif) = Reader::new().read_from_container(&mut bufreader) {
                let lat_val = exif.get_field(Tag::GPSLatitude, In::PRIMARY);
                let lat_ref = exif.get_field(Tag::GPSLatitudeRef, In::PRIMARY).map(|f| f.display_value().to_string());
                let lon_val = exif.get_field(Tag::GPSLongitude, In::PRIMARY);
                let lon_ref = exif.get_field(Tag::GPSLongitudeRef, In::PRIMARY).map(|f| f.display_value().to_string());

                if let (Some(lat_f), Some(lon_f)) = (lat_val, lon_val) {
                    let lat = parse_rational_deg(&lat_f.value, lat_ref.as_deref());
                    let lon = parse_rational_deg(&lon_f.value, lon_ref.as_deref());
                    if let (Some(la), Some(lo)) = (lat, lon) {
                        return Some((la, lo));
                    }
                }
            }
        }
    }

    None
}

pub fn scan_directory(
    source_dir: &Path,
    month_filter: Option<&str>,
) -> Result<(Vec<MediaItem>, usize), String> {
    let source_dir = source_dir.canonicalize().map_err(|e| format!("Invalid path: {}", e))?;
    let protected: HashSet<&str> = [
        "pictures",
        "videos",
        "pictures_doc",
        "trash",
        ".git",
        "_sidecars_archive",
        "node_modules",
    ]
    .iter()
    .cloned()
    .collect();

    let target_month = month_filter
        .map(|m| m.trim().replace('_', "-"))
        .filter(|m| !m.is_empty());

    let mut records = Vec::new();
    let mut total_discovered = 0;

    let walker = WalkDir::new(&source_dir).into_iter().filter_entry(|entry| {
        let name = entry.file_name().to_string_lossy().to_lowercase();
        if entry.file_type().is_dir() && protected.contains(name.as_str()) {
            return false;
        }
        true
    });

    for entry in walker.filter_map(|e| e.ok()) {
        if !entry.file_type().is_file() {
            continue;
        }
        let p = entry.path();
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase();
        if is_media_ext(&ext) {
            total_discovered += 1;
            let sidecar = find_json_sidecar(p);
            let (ts, is_undated) = extract_timestamp(p, sidecar.as_deref());
            let month_str = ts.format("%Y-%m").to_string();

            if let Some(ref tm) = target_month {
                if &month_str != tm {
                    continue;
                }
            }

            let gps = extract_gps_coordinates(p, sidecar.as_deref());
            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
            let is_video = is_video_ext(&ext);

            records.push(MediaItem {
                id: format!("{}_{}", name, size),
                name,
                path: p.to_string_lossy().to_string(),
                ext: format!(".{}", ext),
                is_video,
                size,
                timestamp: ts.to_rfc3339(),
                date_str: ts.format("%Y-%m-%d").to_string(),
                month_str,
                is_undated,
                has_sidecar: sidecar.is_some(),
                sidecar_path: sidecar.map(|s| s.to_string_lossy().to_string()),
                has_gps: gps.is_some(),
                gps,
                category: "PHOTO".to_string(),
                tier: "MIXED".to_string(),
                reason: String::new(),
                caption: String::new(),
                is_cached: None,
            });
        }
    }

    records.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
    Ok((records, total_discovered))
}

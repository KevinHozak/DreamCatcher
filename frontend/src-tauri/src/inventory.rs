use crate::scanner::{scan_directory, MediaItem};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InventoryRecord {
    pub identity: String,
    pub path: String,
    pub root_kind: String,
    pub media_type: String,
    pub extension: String,
    pub size: u64,
    pub timestamp: String,
    pub is_undated: bool,
    pub has_sidecar: bool,
    pub has_gps: bool,
    pub last_seen_scan: String,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InventoryStore {
    pub records: Vec<InventoryRecord>,
    pub scans: Vec<InventoryScanSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InventoryScanSummary {
    pub scan_id: String,
    pub root_kind: String,
    pub root_path: String,
    pub status: String,
    pub discovered: usize,
    pub indexed: usize,
    pub skipped: usize,
    pub completed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InventoryStats {
    pub pictures_count: usize,
    pub pictures_bytes: u64,
    pub videos_count: usize,
    pub videos_bytes: u64,
    pub stale_count: usize,
    pub last_scan: Option<InventoryScanSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InventoryQuery {
    pub root_kind: String,
    pub search: Option<String>,
    pub year: Option<i32>,
    pub date_from: Option<String>,
    pub date_to: Option<String>,
    pub extension: Option<String>,
    pub min_size: Option<u64>,
    pub max_size: Option<u64>,
    pub state: Option<String>,
    pub page: Option<usize>,
    pub page_size: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct InventoryPage {
    pub items: Vec<InventoryRecord>,
    pub total: usize,
    pub page: usize,
    pub page_size: usize,
}

fn inventory_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_data_dir().map_err(|e| format!("Could not locate app data folder: {e}"))?.join("inventory.json"))
}

fn load(path: &Path) -> Result<InventoryStore, String> {
    if !path.exists() { return Ok(InventoryStore::default()); }
    let content = fs::read_to_string(path).map_err(|e| format!("Could not read inventory: {e}"))?;
    serde_json::from_str(&content).map_err(|e| format!("Could not parse inventory: {e}"))
}

fn save(path: &Path, store: &InventoryStore) -> Result<(), String> {
    if let Some(parent) = path.parent() { fs::create_dir_all(parent).map_err(|e| format!("Could not create inventory folder: {e}"))?; }
    let content = serde_json::to_string_pretty(store).map_err(|e| format!("Could not encode inventory: {e}"))?;
    fs::write(path, content).map_err(|e| format!("Could not save inventory: {e}"))
}

fn to_record(item: MediaItem, root_kind: &str, scan_id: &str) -> InventoryRecord {
    InventoryRecord {
        identity: item.path.clone(),
        path: item.path,
        root_kind: root_kind.to_string(),
        media_type: if item.is_video { "video" } else { "picture" }.to_string(),
        extension: item.ext,
        size: item.size,
        timestamp: item.timestamp,
        is_undated: item.is_undated,
        has_sidecar: item.has_sidecar,
        has_gps: item.has_gps,
        last_seen_scan: scan_id.to_string(),
        state: "available".to_string(),
    }
}

pub fn scan(app: &AppHandle, root: String, root_kind: String) -> Result<InventoryScanSummary, String> {
    if root_kind != "pictures" && root_kind != "videos" { return Err("root_kind must be pictures or videos".to_string()); }
    let root_path = PathBuf::from(&root).canonicalize().map_err(|e| format!("Invalid inventory folder: {e}"))?;
    if !root_path.is_dir() { return Err(format!("Inventory folder is not a directory: {root}")); }
    let scan_id = Utc::now().timestamp_nanos_opt().unwrap_or_default().to_string();
    let (items, discovered) = scan_directory(&root_path, None)?;
    let mut store = load(&inventory_path(app)?)?;
    let seen: HashSet<String> = items.iter().map(|item| item.path.clone()).collect();
    for record in store.records.iter_mut().filter(|record| record.root_kind == root_kind && record.state == "available") {
        if !seen.contains(&record.identity) { record.state = "stale".to_string(); }
    }
    let mut indexed = 0;
    for item in items {
        let record = to_record(item, &root_kind, &scan_id);
        if let Some(existing) = store.records.iter_mut().find(|existing| existing.identity == record.identity) {
            *existing = record;
        } else {
            store.records.push(record);
        }
        indexed += 1;
    }
    let summary = InventoryScanSummary { scan_id, root_kind, root_path: root_path.to_string_lossy().to_string(), status: "completed".to_string(), discovered, indexed, skipped: discovered.saturating_sub(indexed), completed_at: Utc::now().to_rfc3339() };
    store.scans.push(summary.clone());
    store.scans.truncate(20);
    save(&inventory_path(app)?, &store)?;
    Ok(summary)
}

pub fn stats(app: &AppHandle) -> Result<InventoryStats, String> {
    let store = load(&inventory_path(app)?)?;
    let mut result = InventoryStats::default();
    for record in &store.records {
        if record.state == "stale" { result.stale_count += 1; continue; }
        match record.media_type.as_str() {
            "picture" => { result.pictures_count += 1; result.pictures_bytes += record.size; }
            "video" => { result.videos_count += 1; result.videos_bytes += record.size; }
            _ => {}
        }
    }
    result.last_scan = store.scans.last().cloned();
    Ok(result)
}

pub fn query(app: &AppHandle, query: InventoryQuery) -> Result<InventoryPage, String> {
    if query.root_kind != "pictures" && query.root_kind != "videos" {
        return Err("root_kind must be pictures or videos".to_string());
    }
    let store = load(&inventory_path(app)?)?;
    let search = query.search.unwrap_or_default().to_lowercase();
    let extension = query.extension.map(|value| {
        let value = value.to_lowercase();
        if value.starts_with('.') { value } else { format!(".{value}") }
    });
    let state = query.state.unwrap_or_else(|| "available".to_string());
    let filtered: Vec<InventoryRecord> = store.records.into_iter().filter(|record| {
        if record.root_kind != query.root_kind || (state == "available" || state == "stale") && record.state != state { return false; }
        if !search.is_empty() && !record.path.to_lowercase().contains(&search) { return false; }
        if let Some(year) = query.year { if !record.timestamp.starts_with(&format!("{year:04}-")) { return false; } }
        if let Some(date_from) = &query.date_from { if record.timestamp < *date_from { return false; } }
        if let Some(date_to) = &query.date_to { if record.timestamp >= *date_to { return false; } }
        if let Some(extension) = &extension { if &record.extension != extension { return false; } }
        if let Some(min_size) = query.min_size { if record.size < min_size { return false; } }
        if let Some(max_size) = query.max_size { if record.size > max_size { return false; } }
        true
    }).collect();
    let page = query.page.unwrap_or(1).max(1);
    let page_size = query.page_size.unwrap_or(50).clamp(1, 100);
    let start = (page - 1) * page_size;
    let total = filtered.len();
    let items = filtered.into_iter().skip(start).take(page_size).collect();
    Ok(InventoryPage { items, total, page, page_size })
}

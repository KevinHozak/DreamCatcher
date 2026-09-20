use crate::inventory::InventoryRecord;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DuplicateMember {
    pub group_id: String,
    pub identity: String,
    pub path: String,
    pub size: u64,
    pub timestamp: String,
    pub excluded: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct DuplicateGroup {
    pub group_id: String,
    pub fingerprint: String,
    pub match_kind: String,
    pub confidence: f64,
    pub created_at: String,
    pub members: Vec<DuplicateMember>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct DuplicateStore {
    groups: Vec<DuplicateGroup>,
}

fn path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Could not locate app data folder: {e}"))?
        .join("duplicates.json"))
}

fn hash_file(path: &Path) -> Result<String, String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let read = file.read(&mut buffer).map_err(|e| e.to_string())?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

fn load(path: &Path) -> Result<DuplicateStore, String> {
    if !path.exists() {
        return Ok(DuplicateStore::default());
    }
    serde_json::from_str(&fs::read_to_string(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

fn save(path: &Path, store: &DuplicateStore) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(
        path,
        serde_json::to_string_pretty(store).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

pub fn analyze(app: &AppHandle) -> Result<serde_json::Value, String> {
    let db = crate::inventory::open_for_app(app)?;
    let mut statement = db.prepare("SELECT identity,path,root_kind,media_type,extension,size,timestamp,is_undated,has_sidecar,has_gps,last_seen_scan,state FROM media_inventory WHERE state='available'").map_err(|e| e.to_string())?;
    let records: Vec<InventoryRecord> = statement
        .query_map([], |row| {
            Ok(InventoryRecord {
                identity: row.get(0)?,
                path: row.get(1)?,
                root_kind: row.get(2)?,
                media_type: row.get(3)?,
                extension: row.get(4)?,
                size: row.get::<_, i64>(5)? as u64,
                timestamp: row.get(6)?,
                is_undated: row.get::<_, i64>(7)? != 0,
                has_sidecar: row.get::<_, i64>(8)? != 0,
                has_gps: row.get::<_, i64>(9)? != 0,
                last_seen_scan: row.get(10)?,
                state: row.get(11)?,
            })
        })
        .map_err(|e| e.to_string())?
        .map(|row| row.map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    let mut candidates: HashMap<String, Vec<InventoryRecord>> = HashMap::new();
    for record in records
        .into_iter()
        .filter(|record| record.state == "available")
    {
        if let Ok(fingerprint) = hash_file(Path::new(&record.path)) {
            candidates.entry(fingerprint).or_default().push(record);
        }
    }
    let groups = candidates
        .into_iter()
        .filter(|(_, members)| members.len() > 1)
        .map(|(fingerprint, records)| {
            let group_id = format!("exact-{fingerprint}");
            DuplicateGroup {
                group_id: group_id.clone(),
                fingerprint,
                match_kind: "exact".to_string(),
                confidence: 1.0,
                created_at: chrono::Utc::now().to_rfc3339(),
                members: records
                    .into_iter()
                    .map(|record| DuplicateMember {
                        group_id: group_id.clone(),
                        identity: record.identity,
                        path: record.path,
                        size: record.size,
                        timestamp: record.timestamp,
                        excluded: false,
                    })
                    .collect(),
            }
        })
        .collect::<Vec<_>>();
    let summary = serde_json::json!({"groups": groups.len(), "members": groups.iter().map(|group| group.members.len()).sum::<usize>(), "match_kind": "exact"});
    save(&path(app)?, &DuplicateStore { groups })?;
    Ok(summary)
}

pub fn list(app: &AppHandle) -> Result<serde_json::Value, String> {
    let store = load(&path(app)?)?;
    Ok(
        serde_json::json!({"groups": store.groups, "total": store.groups.len(), "page": 1, "page_size": 100}),
    )
}

pub fn exclude(
    app: &AppHandle,
    group_id: String,
    identity: String,
    excluded: bool,
) -> Result<(), String> {
    let mut store = load(&path(app)?)?;
    let mut found = false;
    for group in &mut store.groups {
        if group.group_id == group_id {
            for member in &mut group.members {
                if member.identity == identity {
                    member.excluded = excluded;
                    found = true;
                }
            }
        }
    }
    if !found {
        return Err("Duplicate member was not found".to_string());
    }
    save(&path(app)?, &store)
}

use crate::inventory::InventoryRecord;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager};

// Serialize review-store reads/writes on background workers to avoid lost exclusions.
static REVIEW_STORE: std::sync::Mutex<()> = std::sync::Mutex::new(());

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
    analyze_with_progress(&db, &path(app)?, |processed, total| {
        let _ = app.emit(
            "duplicate-analysis-progress",
            serde_json::json!({"processed": processed, "total": total}),
        );
    })
}

#[cfg(test)]
fn analyze_connection(
    db: &rusqlite::Connection,
    store_path: &Path,
) -> Result<serde_json::Value, String> {
    analyze_with_progress(db, store_path, |_, _| {})
}
fn analyze_with_progress(
    db: &rusqlite::Connection,
    store_path: &Path,
    progress: impl Fn(usize, usize),
) -> Result<serde_json::Value, String> {
    let _guard = REVIEW_STORE.lock().map_err(|e| e.to_string())?;
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
                ..Default::default()
            })
        })
        .map_err(|e| e.to_string())?
        .map(|row| row.map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    let mut candidates: HashMap<String, Vec<InventoryRecord>> = HashMap::new();
    let total = records.len();
    progress(0, total);
    for (index, record) in records.into_iter().enumerate() {
        if let Ok(fingerprint) = hash_file(Path::new(&record.path)) {
            candidates.entry(fingerprint).or_default().push(record);
        }
        if (index + 1) % 100 == 0 || index + 1 == total {
            progress(index + 1, total);
        }
    }
    let previous = load(store_path)?;
    let exclusions: std::collections::HashSet<(String, String)> = previous
        .groups
        .into_iter()
        .flat_map(|g| g.members)
        .filter(|m| m.excluded)
        .map(|m| (m.group_id, m.identity))
        .collect();
    let mut groups = candidates
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
                        excluded: exclusions.contains(&(group_id.clone(), record.identity.clone())),
                        identity: record.identity,
                        path: record.path,
                        size: record.size,
                        timestamp: record.timestamp,
                    })
                    .collect(),
            }
        })
        .collect::<Vec<_>>();
    groups.sort_by(|a, b| a.group_id.cmp(&b.group_id));
    let summary = serde_json::json!({"groups": groups.len(), "members": groups.iter().map(|group| group.members.len()).sum::<usize>(), "match_kind": "exact"});
    save(store_path, &DuplicateStore { groups })?;
    Ok(summary)
}

pub fn list(app: &AppHandle, page: usize) -> Result<serde_json::Value, String> {
    list_path(&path(app)?, page)
}
fn list_path(store_path: &Path, page: usize) -> Result<serde_json::Value, String> {
    let _guard = REVIEW_STORE.lock().map_err(|e| e.to_string())?;
    let store = load(store_path)?;
    let page = page.max(1);
    let total = store.groups.len();
    let groups: Vec<_> = store
        .groups
        .into_iter()
        .skip(page.saturating_sub(1).saturating_mul(50))
        .take(50)
        .collect();
    Ok(serde_json::json!({"groups": groups, "total": total, "page": page, "page_size": 50}))
}
pub fn proposal(app: &AppHandle) -> Result<serde_json::Value, String> {
    proposal_path(&path(app)?)
}
fn proposal_path(store_path: &Path) -> Result<serde_json::Value, String> {
    let _guard = REVIEW_STORE.lock().map_err(|e| e.to_string())?;
    let store = load(store_path)?;
    Ok(
        serde_json::json!({"version": 1, "generated_at": chrono::Utc::now().to_rfc3339(), "mode": "review-only", "groups": store.groups}),
    )
}

pub fn exclude(
    app: &AppHandle,
    group_id: String,
    identity: String,
    excluded: bool,
) -> Result<(), String> {
    exclude_path(&path(app)?, &group_id, &identity, excluded)
}
fn exclude_path(
    store_path: &Path,
    group_id: &str,
    identity: &str,
    excluded: bool,
) -> Result<(), String> {
    let _guard = REVIEW_STORE.lock().map_err(|e| e.to_string())?;
    let mut store = load(store_path)?;
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
    save(store_path, &store)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::{params, Connection};
    use std::time::Instant;

    #[cfg(windows)]
    fn peak_working_set() -> usize {
        #[repr(C)]
        struct Counters {
            cb: u32,
            faults: u32,
            peak: usize,
            working: usize,
            peak_paged: usize,
            paged: usize,
            peak_nonpaged: usize,
            nonpaged: usize,
            pagefile: usize,
            peak_pagefile: usize,
        }
        #[link(name = "psapi")]
        extern "system" {
            fn GetProcessMemoryInfo(
                process: *mut std::ffi::c_void,
                counters: *mut Counters,
                size: u32,
            ) -> i32;
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn GetCurrentProcess() -> *mut std::ffi::c_void;
        }
        let mut counters: Counters = unsafe { std::mem::zeroed() };
        counters.cb = std::mem::size_of::<Counters>() as u32;
        assert_ne!(
            unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) },
            0
        );
        counters.peak
    }
    #[cfg(not(windows))]
    fn peak_working_set() -> usize {
        0
    }

    fn exercise(count: usize, analyzing: &std::sync::atomic::AtomicBool) -> serde_json::Value {
        let root = std::env::temp_dir().join(format!(
            "dc-duplicate-bench-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap()
        ));
        fs::create_dir(&root).unwrap();
        let media = root.join("media");
        fs::create_dir(&media).unwrap();
        let store = root.join("duplicates.json");
        let mut db = Connection::open(root.join("inventory.sqlite3")).unwrap();
        db.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE media_inventory(identity TEXT PRIMARY KEY,path TEXT,root_kind TEXT,media_type TEXT,extension TEXT,size INTEGER,timestamp TEXT,is_undated INTEGER,has_sidecar INTEGER,has_gps INTEGER,last_seen_scan TEXT,state TEXT);").unwrap();
        let started = Instant::now();
        let mut expected = Vec::new();
        for index in 0..count {
            let mut content = vec![0x5a; 4096];
            content[..8].copy_from_slice(&((index / 2) as u64).to_le_bytes());
            let file = media.join(format!("{index:06}.jpg"));
            fs::write(&file, &content).unwrap();
            expected.push((file, format!("{:x}", Sha256::digest(&content))));
        }
        let sidecar = media.join("sidecar.json");
        fs::write(&sidecar, b"{\"description\":\"preserve me\"}").unwrap();
        let fixture_ms = started.elapsed().as_secs_f64() * 1000.0;
        let started = Instant::now();
        {
            let tx = db.transaction().unwrap();
            let mut insert = tx.prepare("INSERT INTO media_inventory VALUES(?,?,'pictures','picture','.jpg',4096,'2026-01-01T00:00:00Z',0,0,0,'fixture','available')").unwrap();
            for (index, (file, _)) in expected.iter().enumerate() {
                insert
                    .execute(params![index.to_string(), file.to_string_lossy()])
                    .unwrap();
            }
            drop(insert);
            tx.commit().unwrap();
        }
        let sqlite_index_ms = started.elapsed().as_secs_f64() * 1000.0;
        let started = Instant::now();
        analyzing.store(true, std::sync::atomic::Ordering::SeqCst);
        let batches = std::cell::Cell::new(0usize);
        let summary = analyze_with_progress(&db, &store, |processed, total| {
            assert!(processed <= total);
            batches.set(batches.get() + 1);
        })
        .unwrap();
        analyzing.store(false, std::sync::atomic::Ordering::SeqCst);
        assert_eq!(batches.get(), count.div_ceil(100) + 1);
        let analysis_ms = started.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(summary["groups"], count / 2);
        assert_eq!(summary["members"], count);
        let started = Instant::now();
        let page = list_path(&store, 1).unwrap();
        let review_page_ms = started.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(
            page["groups"].as_array().unwrap().len(),
            (count / 2).min(50)
        );
        assert_eq!(page["total"], count / 2);
        let group = page["groups"][0]["group_id"].as_str().unwrap();
        let identity = page["groups"][0]["members"][0]["identity"]
            .as_str()
            .unwrap();
        let started = Instant::now();
        exclude_path(&store, group, identity, true).unwrap();
        let exclusion_ms = started.elapsed().as_secs_f64() * 1000.0;
        assert!(exclude_path(&store, group, "missing", true).is_err());
        let started = Instant::now();
        let proposal = proposal_path(&store).unwrap();
        let encoded = serde_json::to_vec(&proposal).unwrap();
        let export_ms = started.elapsed().as_secs_f64() * 1000.0;
        assert_eq!(proposal["mode"], "review-only");
        assert_eq!(proposal["groups"].as_array().unwrap().len(), count / 2);
        assert_eq!(proposal["groups"][0]["members"][0]["excluded"], true);
        analyze_connection(&db, &store).unwrap();
        assert_eq!(
            list_path(&store, 1).unwrap()["groups"][0]["members"][0]["excluded"],
            true
        );
        exclude_path(&store, group, identity, false).unwrap();
        assert_eq!(
            list_path(&store, 1).unwrap()["groups"][0]["members"][0]["excluded"],
            false
        );
        let last = list_path(&store, (count / 2).div_ceil(50)).unwrap();
        assert!(!last["groups"].as_array().unwrap().is_empty());
        // Check every original byte after analysis, toggle, export, and reanalysis.
        for (file, digest) in &expected {
            assert_eq!(&hash_file(file).unwrap(), digest);
        }
        assert_eq!(
            fs::read(&sidecar).unwrap(),
            b"{\"description\":\"preserve me\"}"
        );
        assert_eq!(fs::read_dir(&media).unwrap().count(), count + 1);
        let result = serde_json::json!({"files": count, "bytes_per_file": 4096, "groups": count/2, "fixture_ms": fixture_ms, "sqlite_index_ms": sqlite_index_ms, "analysis_ms": analysis_ms, "review_page_ms": review_page_ms, "exclusion_ms": exclusion_ms, "export_ms": export_ms, "proposal_bytes": encoded.len(), "peak_working_set_bytes": peak_working_set(), "unchanged_media_files": count, "unchanged_sidecars": 1});
        drop(db);
        assert!(root.starts_with(std::env::temp_dir()));
        fs::remove_dir_all(root).unwrap();
        result
    }

    #[test]
    fn review_and_export_preserve_media_and_exclusions() {
        exercise(240, &std::sync::atomic::AtomicBool::new(false));
    }

    #[test]
    #[ignore = "Explicit scale benchmark creates thousands of disposable synthetic files"]
    fn benchmark_exact_duplicates() {
        let count = std::env::var("DREAMCATCHER_BENCH_FILES")
            .unwrap_or_else(|_| "5000".into())
            .parse::<usize>()
            .unwrap();
        assert!(count >= 2 && count <= 50000 && count % 2 == 0);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .unwrap();
        let mut result = runtime.block_on(async {
            let analyzing = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
            let worker_flag = analyzing.clone();
            let mut worker = tokio::task::spawn_blocking(move || exercise(count, &worker_flag));
            let mut ticks = 0usize;
            let mut max_gap_ms = 0.0f64;
            let mut previous = Instant::now();
            let mut interval = tokio::time::interval(std::time::Duration::from_millis(16));
            loop {
                tokio::select! {
                    result = &mut worker => {
                        let mut result = result.unwrap();
                        result["analysis_scheduler_ticks"] = ticks.into();
                        result["analysis_scheduler_max_gap_ms"] = max_gap_ms.into();
                        assert!(ticks > 0, "Analysis must allow runtime heartbeat ticks");
                        break result;
                    }
                    _ = interval.tick() => {
                        let now = Instant::now();
                        if analyzing.load(std::sync::atomic::Ordering::SeqCst) {
                            ticks += 1;
                            max_gap_ms = max_gap_ms.max(now.duration_since(previous).as_secs_f64() * 1000.0);
                        }
                        previous = now;
                    }
                }
            }
        });
        result["profile"] = "debug".into();
        println!("DUPLICATE_BENCHMARK {}", result);
    }
}

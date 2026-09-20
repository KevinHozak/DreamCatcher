use crate::scanner::{scan_directory, MediaItem};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager};

const SCHEMA_VERSION: i64 = 1;

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
#[derive(Debug, Deserialize, Default)]
struct LegacyInventoryStore {
    records: Vec<InventoryRecord>,
    scans: Vec<InventoryScanSummary>,
}

pub fn inventory_path(_app: &AppHandle) -> Result<PathBuf, String> {
    if let Ok(path) = std::env::var("DREAMCATCHER_INVENTORY_PATH") {
        return Ok(PathBuf::from(path));
    }
    let root = if cfg!(windows) {
        std::env::var("LOCALAPPDATA")
            .map(PathBuf::from)
            .map_err(|_| "LOCALAPPDATA is not available".to_string())?
    } else {
        PathBuf::from(std::env::var("XDG_DATA_HOME").unwrap_or_else(|_| {
            format!("{}/.local/share", std::env::var("HOME").unwrap_or_default())
        }))
    };
    Ok(root.join("DreamCatcher").join("inventory.sqlite3"))
}

fn legacy_path(db_path: &Path, app: &AppHandle) -> Result<PathBuf, String> {
    let app_path = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("Could not locate app data folder: {e}"))?
        .join("inventory.json");
    Ok(if app_path.exists() {
        app_path
    } else {
        db_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("inventory.json")
    })
}

fn migrate_json(conn: &Connection, json_path: &Path) -> Result<(), String> {
    let content = fs::read_to_string(json_path)
        .map_err(|e| format!("Could not read legacy inventory for migration: {e}"))?;
    let legacy: LegacyInventoryStore = serde_json::from_str(&content)
        .map_err(|e| format!("Could not parse legacy inventory; no records were migrated: {e}"))?;
    let backup = json_path.with_extension(format!(
        "json.migrated-{}.bak",
        Utc::now().format("%Y%m%d%H%M%S")
    ));
    fs::copy(json_path, &backup)
        .map_err(|e| format!("Could not create legacy inventory backup: {e}"))?;
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    for r in legacy.records {
        tx.execute("INSERT OR IGNORE INTO media_inventory(identity,path,root_kind,media_type,extension,size,timestamp,is_undated,has_sidecar,has_gps,last_seen_scan,state) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)", params![r.identity,r.path,r.root_kind,r.media_type,r.extension,r.size as i64,r.timestamp,r.is_undated as i64,r.has_sidecar as i64,r.has_gps as i64,r.last_seen_scan,r.state]).map_err(|e| e.to_string())?;
    }
    for s in legacy.scans {
        tx.execute("INSERT OR IGNORE INTO inventory_scans(scan_id,root_kind,root_path,started_at,completed_at,status,discovered,indexed,skipped) VALUES(?,?,?,?,?,?,?,?,?)", params![s.scan_id,s.root_kind,s.root_path,s.completed_at,s.completed_at,s.status,s.discovered as i64,s.indexed as i64,s.skipped as i64]).map_err(|e| e.to_string())?;
    }
    tx.commit().map_err(|e| e.to_string())?;
    conn.execute(
        "INSERT OR REPLACE INTO inventory_meta(key,value) VALUES('legacy_migration_backup',?)",
        [backup.to_string_lossy().to_string()],
    )
    .map_err(|e| e.to_string())?;
    Ok(())
}

fn connect(path: &Path, legacy: Option<&Path>) -> Result<Connection, String> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Could not create inventory folder: {e}"))?;
    }
    let conn =
        Connection::open(path).map_err(|e| format!("Could not open inventory database: {e}"))?;
    conn.busy_timeout(std::time::Duration::from_secs(10))
        .map_err(|e| e.to_string())?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|e| e.to_string())?;
    conn.execute_batch("CREATE TABLE IF NOT EXISTS inventory_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL); CREATE TABLE IF NOT EXISTS media_inventory (identity TEXT PRIMARY KEY,path TEXT NOT NULL,root_kind TEXT NOT NULL,media_type TEXT NOT NULL,extension TEXT NOT NULL,size INTEGER NOT NULL,timestamp TEXT NOT NULL,is_undated INTEGER NOT NULL,has_sidecar INTEGER NOT NULL,has_gps INTEGER NOT NULL,last_seen_scan TEXT NOT NULL,state TEXT NOT NULL DEFAULT 'available'); CREATE INDEX IF NOT EXISTS idx_inventory_type ON media_inventory(media_type); CREATE INDEX IF NOT EXISTS idx_inventory_timestamp ON media_inventory(timestamp); CREATE INDEX IF NOT EXISTS idx_inventory_state ON media_inventory(state); CREATE TABLE IF NOT EXISTS inventory_scans (scan_id TEXT PRIMARY KEY,root_kind TEXT NOT NULL,root_path TEXT NOT NULL,started_at TEXT NOT NULL,completed_at TEXT,status TEXT NOT NULL,discovered INTEGER NOT NULL DEFAULT 0,indexed INTEGER NOT NULL DEFAULT 0,skipped INTEGER NOT NULL DEFAULT 0,error TEXT); CREATE TABLE IF NOT EXISTS inventory_diagnostics (id INTEGER PRIMARY KEY AUTOINCREMENT,scan_id TEXT NOT NULL,path TEXT,message TEXT NOT NULL,created_at TEXT NOT NULL);").map_err(|e| format!("Could not initialize inventory schema: {e}"))?;
    let version: Option<i64> = conn
        .query_row(
            "SELECT value FROM inventory_meta WHERE key='schema_version'",
            [],
            |r| r.get::<_, String>(0),
        )
        .optional()
        .map_err(|e| e.to_string())?
        .and_then(|v| v.parse().ok());
    if version.unwrap_or(0) > SCHEMA_VERSION {
        return Err(format!(
            "Inventory database schema {} is newer than supported schema {SCHEMA_VERSION}",
            version.unwrap()
        ));
    }
    if version.unwrap_or(0) == 0 {
        if let Some(json) = legacy.filter(|p| p.exists()) {
            migrate_json(&conn, json)?;
        }
    }
    conn.execute(
        "INSERT OR REPLACE INTO inventory_meta(key,value) VALUES('schema_version',?)",
        [SCHEMA_VERSION.to_string()],
    )
    .map_err(|e| e.to_string())?;
    Ok(conn)
}

fn identity_for(path: &str, size: u64) -> String {
    let modified = Path::new(path)
        .metadata()
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let mut digest = Sha256::new();
    digest.update(format!("{}|{}|{}", path, size, modified).as_bytes());
    format!("{:x}", digest.finalize())
}
fn to_record(item: MediaItem, root_kind: &str, scan_id: &str) -> InventoryRecord {
    let identity = identity_for(&item.path, item.size);
    InventoryRecord {
        identity,
        path: item.path,
        root_kind: root_kind.into(),
        media_type: if item.is_video { "video" } else { "picture" }.into(),
        extension: item.ext,
        size: item.size,
        timestamp: item.timestamp,
        is_undated: item.is_undated,
        has_sidecar: item.has_sidecar,
        has_gps: item.has_gps,
        last_seen_scan: scan_id.into(),
        state: "available".into(),
    }
}
fn from_row(r: &rusqlite::Row<'_>) -> rusqlite::Result<InventoryRecord> {
    Ok(InventoryRecord {
        identity: r.get(0)?,
        path: r.get(1)?,
        root_kind: r.get(2)?,
        media_type: r.get(3)?,
        extension: r.get(4)?,
        size: r.get::<_, i64>(5)? as u64,
        timestamp: r.get(6)?,
        is_undated: r.get::<_, i64>(7)? != 0,
        has_sidecar: r.get::<_, i64>(8)? != 0,
        has_gps: r.get::<_, i64>(9)? != 0,
        last_seen_scan: r.get(10)?,
        state: r.get(11)?,
    })
}
pub fn open_for_app(app: &AppHandle) -> Result<Connection, String> {
    let p = inventory_path(app)?;
    let l = legacy_path(&p, app)?;
    connect(&p, Some(&l))
}

pub fn scan(
    app: &AppHandle,
    root: String,
    root_kind: String,
) -> Result<InventoryScanSummary, String> {
    if root_kind != "pictures" && root_kind != "videos" {
        return Err("root_kind must be pictures or videos".into());
    }
    let root_path = PathBuf::from(&root)
        .canonicalize()
        .map_err(|e| format!("Invalid inventory folder: {e}"))?;
    if !root_path.is_dir() {
        return Err(format!("Inventory folder is not a directory: {root}"));
    }
    let p = inventory_path(app)?;
    let l = legacy_path(&p, app)?;
    let conn = connect(&p, Some(&l))?;
    let scan_id = Utc::now()
        .timestamp_nanos_opt()
        .unwrap_or_default()
        .to_string();
    let started = Utc::now().to_rfc3339();
    conn.execute("INSERT INTO inventory_scans(scan_id,root_kind,root_path,started_at,status) VALUES(?,?,?,?,?)",params![scan_id,root_kind,root_path.to_string_lossy(),started,"running"]).map_err(|e|e.to_string())?;
    let (items, discovered) = scan_directory(&root_path, None)?;
    let tx = conn.unchecked_transaction().map_err(|e| e.to_string())?;
    for item in items {
        let r = to_record(item, &root_kind, &scan_id);
        tx.execute("INSERT INTO media_inventory(identity,path,root_kind,media_type,extension,size,timestamp,is_undated,has_sidecar,has_gps,last_seen_scan,state) VALUES(?,?,?,?,?,?,?,?,?,?,?,?) ON CONFLICT(identity) DO UPDATE SET path=excluded.path,root_kind=excluded.root_kind,media_type=excluded.media_type,extension=excluded.extension,size=excluded.size,timestamp=excluded.timestamp,is_undated=excluded.is_undated,has_sidecar=excluded.has_sidecar,has_gps=excluded.has_gps,last_seen_scan=excluded.last_seen_scan,state='available'",params![r.identity,r.path,r.root_kind,r.media_type,r.extension,r.size as i64,r.timestamp,r.is_undated as i64,r.has_sidecar as i64,r.has_gps as i64,r.last_seen_scan,r.state]).map_err(|e|e.to_string())?;
    }
    tx.execute("UPDATE media_inventory SET state='stale' WHERE root_kind=? AND state='available' AND last_seen_scan<>? AND path LIKE ?",params![root_kind,scan_id,format!("{}{}%",root_path.to_string_lossy(),std::path::MAIN_SEPARATOR)]).map_err(|e|e.to_string())?;
    let completed = Utc::now().to_rfc3339();
    tx.execute("UPDATE inventory_scans SET completed_at=?,status='completed',discovered=?,indexed=?,skipped=0 WHERE scan_id=?",params![completed,discovered as i64,discovered as i64,scan_id]).map_err(|e|e.to_string())?;
    tx.commit().map_err(|e| e.to_string())?;
    Ok(InventoryScanSummary {
        scan_id,
        root_kind,
        root_path: root_path.to_string_lossy().into(),
        status: "completed".into(),
        discovered,
        indexed: discovered,
        skipped: 0,
        completed_at: completed,
    })
}

pub fn stats(app: &AppHandle) -> Result<InventoryStats, String> {
    let conn = open_for_app(app)?;
    let mut out = InventoryStats::default();
    let mut st=conn.prepare("SELECT media_type,state,COUNT(*),COALESCE(SUM(size),0) FROM media_inventory GROUP BY media_type,state").map_err(|e|e.to_string())?;
    for row in st
        .query_map([], |r| {
            Ok((
                r.get::<_, String>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })
        .map_err(|e| e.to_string())?
    {
        let (k, s, n, b) = row.map_err(|e| e.to_string())?;
        if s == "stale" {
            out.stale_count += n as usize
        } else if k == "picture" {
            out.pictures_count += n as usize;
            out.pictures_bytes += b as u64
        } else if k == "video" {
            out.videos_count += n as usize;
            out.videos_bytes += b as u64
        }
    }
    out.last_scan=conn.query_row("SELECT scan_id,root_kind,root_path,status,discovered,indexed,skipped,COALESCE(completed_at,'') FROM inventory_scans ORDER BY started_at DESC LIMIT 1",[],|r|Ok(InventoryScanSummary{scan_id:r.get(0)?,root_kind:r.get(1)?,root_path:r.get(2)?,status:r.get(3)?,discovered:r.get::<_,i64>(4)? as usize,indexed:r.get::<_,i64>(5)? as usize,skipped:r.get::<_,i64>(6)? as usize,completed_at:r.get(7)?})).optional().map_err(|e|e.to_string())?;
    Ok(out)
}

pub fn query(app: &AppHandle, q: InventoryQuery) -> Result<InventoryPage, String> {
    if q.root_kind != "pictures" && q.root_kind != "videos" {
        return Err("root_kind must be pictures or videos".into());
    };
    let conn = open_for_app(app)?;
    let page = q.page.unwrap_or(1).max(1);
    let page_size = q.page_size.unwrap_or(50).clamp(1, 100);
    let state = q.state.unwrap_or_else(|| "available".into());
    let mut sql: String="SELECT identity,path,root_kind,media_type,extension,size,timestamp,is_undated,has_sidecar,has_gps,last_seen_scan,state FROM media_inventory WHERE root_kind=?".into();
    let mut args: Vec<Box<dyn rusqlite::ToSql>> = vec![Box::new(q.root_kind)];
    if state == "available" || state == "stale" {
        sql.push_str(" AND state=?");
        args.push(Box::new(state));
    }
    if let Some(v) = q.search.filter(|s| !s.trim().is_empty()) {
        sql.push_str(" AND LOWER(path) LIKE ?");
        args.push(Box::new(format!("%{}%", v.to_lowercase())))
    }
    if let Some(v) = q.year {
        sql.push_str(" AND timestamp LIKE ?");
        args.push(Box::new(format!("{v:04}-%")))
    }
    if let Some(v) = q.date_from {
        sql.push_str(" AND timestamp>=?");
        args.push(Box::new(v))
    }
    if let Some(v) = q.date_to {
        sql.push_str(" AND timestamp<?");
        args.push(Box::new(v))
    }
    if let Some(v) = q.extension {
        let e = if v.starts_with('.') {
            v.to_lowercase()
        } else {
            format!(".{}", v.to_lowercase())
        };
        sql.push_str(" AND extension=?");
        args.push(Box::new(e))
    }
    if let Some(v) = q.min_size {
        sql.push_str(" AND size>=?");
        args.push(Box::new(v as i64))
    }
    if let Some(v) = q.max_size {
        sql.push_str(" AND size<=?");
        args.push(Box::new(v as i64))
    }
    let total = conn
        .query_row(
            &format!("SELECT COUNT(*) FROM ({sql})"),
            rusqlite::params_from_iter(args.iter().map(|v| v.as_ref())),
            |r| r.get::<_, i64>(0),
        )
        .map_err(|e| e.to_string())? as usize;
    sql.push_str(" ORDER BY timestamp DESC,path LIMIT ? OFFSET ?");
    args.push(Box::new(page_size as i64));
    args.push(Box::new(((page - 1) * page_size) as i64));
    let mut st = conn.prepare(&sql).map_err(|e| e.to_string())?;
    let items = st
        .query_map(
            rusqlite::params_from_iter(args.iter().map(|v| v.as_ref())),
            from_row,
        )
        .map_err(|e| e.to_string())?
        .map(|v| v.map_err(|e| e.to_string()))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(InventoryPage {
        items,
        total,
        page,
        page_size,
    })
}

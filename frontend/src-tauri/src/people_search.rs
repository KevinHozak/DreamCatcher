use serde::Serialize;
use std::fs;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Serialize)]
pub struct PeopleSearchStatus {
    pub enabled: bool,
    pub state: String,
    pub message: String,
    pub index_exists: bool,
    pub indexed_media: usize,
    pub reviewed_people: usize,
    pub last_run: Option<String>,
}

fn index_path(app: &AppHandle) -> Result<PathBuf, String> {
    Ok(app.path().app_data_dir().map_err(|e| format!("Could not locate app data folder: {e}"))?.join("people-index.sqlite3"))
}

#[tauri::command]
pub fn status(app: &AppHandle, enabled: bool) -> Result<PeopleSearchStatus, String> {
    let p = index_path(app)?;
    Ok(status_for_index_path(&p, enabled))
}

pub fn status_for_index_path(index_path: &std::path::Path, enabled: bool) -> PeopleSearchStatus {
    let exists = index_path.exists();
    PeopleSearchStatus {
        enabled,
        // A database file alone does not prove that an approved runtime has
        // produced a usable index. No native face-indexing runtime is shipped.
        state: if !enabled { "disabled" } else { "not_ready" }.to_string(),
        message: if !enabled {
            "People search is disabled. No face processing or face-derived data is created.".to_string()
        } else {
            "People search is enabled, but indexing is unavailable because no vetted local face runtime is configured. An existing derived database is not a usable index.".to_string()
        },
        index_exists: exists,
        indexed_media: 0,
        reviewed_people: 0,
        last_run: None,
    }
}

#[tauri::command]
pub fn delete_index(app: &AppHandle) -> Result<serde_json::Value, String> {
    let path = index_path(app)?;
    delete_index_at_path(&path)
}

pub fn delete_index_at_path(path: &std::path::Path) -> Result<serde_json::Value, String> {
    let base_str = path.to_string_lossy();
    for suffix in ["", "-wal", "-shm"] {
        let candidate = PathBuf::from(format!("{base_str}{suffix}"));
        if candidate.exists() {
            fs::remove_file(&candidate).map_err(|e| format!("Could not delete people-search file {}: {e}", candidate.display()))?;
        }
    }
    Ok(serde_json::json!({ "deleted": true }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_people_search_status_never_claims_ready_and_cleans_sidecars() {
        let dir = std::env::temp_dir().join(format!("dc-people-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let db_path = dir.join("people-index.sqlite3");
        let wal_path = dir.join("people-index.sqlite3-wal");
        let shm_path = dir.join("people-index.sqlite3-shm");

        // 1. When disabled and no db exists
        let st_disabled = status_for_index_path(&db_path, false);
        assert!(!st_disabled.enabled);
        assert_eq!(st_disabled.state, "disabled");
        assert!(!st_disabled.index_exists);
        assert_eq!(st_disabled.indexed_media, 0);
        assert_eq!(st_disabled.reviewed_people, 0);

        // 2. When disabled but old db file exists
        fs::write(&db_path, b"mock-legacy-face-db").unwrap();
        let st_disabled_with_db = status_for_index_path(&db_path, false);
        assert_eq!(st_disabled_with_db.state, "disabled");
        assert!(st_disabled_with_db.index_exists);
        assert_eq!(st_disabled_with_db.indexed_media, 0);

        // 3. When enabled with old derived db file
        let st_enabled = status_for_index_path(&db_path, true);
        assert!(st_enabled.enabled);
        assert_eq!(st_enabled.state, "not_ready");
        assert!(st_enabled.index_exists);
        assert_eq!(st_enabled.indexed_media, 0);
        assert_eq!(st_enabled.reviewed_people, 0);
        assert!(st_enabled.message.contains("no vetted local face runtime is configured"));

        // 4. Test delete_index removes sqlite, wal, and shm
        fs::write(&wal_path, b"wal-data").unwrap();
        fs::write(&shm_path, b"shm-data").unwrap();
        assert!(db_path.exists());
        assert!(wal_path.exists());
        assert!(shm_path.exists());

        let res = delete_index_at_path(&db_path).expect("delete index failed");
        assert_eq!(res.get("deleted").and_then(|v| v.as_bool()), Some(true));
        assert!(!db_path.exists());
        assert!(!wal_path.exists());
        assert!(!shm_path.exists());

        let _ = fs::remove_dir_all(dir);
    }
}

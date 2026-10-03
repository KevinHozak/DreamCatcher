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
    let exists = index_path(app)?.exists();
    Ok(PeopleSearchStatus {
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
    })
}

#[tauri::command]
pub fn delete_index(app: &AppHandle) -> Result<serde_json::Value, String> {
    let path = index_path(app)?;
    let base_str = path.to_string_lossy();
    for suffix in ["", "-wal", "-shm"] {
        let candidate = PathBuf::from(format!("{base_str}{suffix}"));
        if candidate.exists() {
            fs::remove_file(&candidate).map_err(|e| format!("Could not delete people-search file {}: {e}", candidate.display()))?;
        }
    }
    Ok(serde_json::json!({ "deleted": true }))
}

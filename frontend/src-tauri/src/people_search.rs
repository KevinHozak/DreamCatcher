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
        state: if !enabled { "disabled" } else if exists { "ready" } else { "not_ready" }.to_string(),
        message: if !enabled {
            "People search is disabled. No face processing or face-derived data is created.".to_string()
        } else if exists {
            "People search is available for reviewed local labels.".to_string()
        } else {
            "People search is enabled, but no vetted local face runtime is configured yet.".to_string()
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
    if path.exists() {
        fs::remove_file(path).map_err(|e| format!("Could not delete people-search index: {e}"))?;
    }
    Ok(serde_json::json!({ "deleted": true }))
}

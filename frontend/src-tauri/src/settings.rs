use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

pub const DEFAULT_SOURCE_DIR: &str = r"C:\Transfer\Takeout\K Photos\2024";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppSettings {
    pub source_dir: String,
    pub pictures_dir: Option<String>,
    pub videos_dir: Option<String>,
    #[serde(default)]
    pub people_search_enabled: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            source_dir: DEFAULT_SOURCE_DIR.to_string(),
            pictures_dir: None,
            videos_dir: None,
            people_search_enabled: false,
        }
    }
}

fn clean_optional_path(value: Option<String>) -> Option<String> {
    value.and_then(|path| {
        let trimmed = path.trim().to_string();
        if trimmed.is_empty() { None } else { Some(trimmed) }
    })
}

fn validate_directory(label: &str, value: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(value.trim());
    if !path.is_absolute() {
        return Err(format!("{label} must be an absolute path."));
    }
    if !path.exists() {
        return Err(format!("{label} does not exist: {value}"));
    }
    if !path.is_dir() {
        return Err(format!("{label} must be a directory: {value}"));
    }
    path.canonicalize()
        .map_err(|_| format!("{label} is not accessible: {value}"))
}

pub fn validate_settings(settings: &AppSettings) -> Result<AppSettings, String> {
    let source_dir = settings.source_dir.trim();
    if source_dir.is_empty() {
        return Err("Source folder is required.".to_string());
    }
    validate_directory("Source folder", source_dir)?;

    let pictures_dir = clean_optional_path(settings.pictures_dir.clone());
    let videos_dir = clean_optional_path(settings.videos_dir.clone());
    let pictures = pictures_dir
        .as_deref()
        .map(|path| validate_directory("Pictures folder", path))
        .transpose()?;
    let videos = videos_dir
        .as_deref()
        .map(|path| validate_directory("Videos folder", path))
        .transpose()?;

    if let (Some(pictures), Some(videos)) = (&pictures, &videos) {
        if pictures == videos {
            return Err("Pictures and Videos folders must be different directories.".to_string());
        }
        if pictures.starts_with(videos) || videos.starts_with(pictures) {
            return Err("Pictures and Videos folders cannot contain one another.".to_string());
        }
    }

    Ok(AppSettings {
        source_dir: source_dir.to_string(),
        pictures_dir,
        videos_dir,
        people_search_enabled: settings.people_search_enabled,
    })
}

pub fn load(path: &Path) -> Result<AppSettings, String> {
    if !path.exists() {
        return Ok(AppSettings::default());
    }
    let contents = fs::read_to_string(path).map_err(|e| format!("Could not read settings: {e}"))?;
    serde_json::from_str(&contents).map_err(|e| format!("Could not parse settings: {e}"))
}

pub fn save(path: &Path, settings: &AppSettings) -> Result<AppSettings, String> {
    let validated = validate_settings(settings)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| format!("Could not create settings folder: {e}"))?;
    }
    let contents = serde_json::to_string_pretty(&validated)
        .map_err(|e| format!("Could not encode settings: {e}"))?;
    fs::write(path, contents).map_err(|e| format!("Could not save settings: {e}"))?;
    Ok(validated)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn allows_independent_optional_destinations() {
        let root = std::env::temp_dir().join(format!("dreamcatcher-settings-test-{}", std::process::id()));
        let source = root.join("source");
        let pictures = root.join("pictures");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&pictures).unwrap();

        let result = validate_settings(&AppSettings {
            source_dir: source.to_string_lossy().to_string(),
            pictures_dir: Some(pictures.to_string_lossy().to_string()),
            videos_dir: None,
            people_search_enabled: false,
        });
        assert!(result.is_ok());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_nested_destinations() {
        let root = std::env::temp_dir().join(format!("dreamcatcher-settings-nested-{}", std::process::id()));
        let source = root.join("source");
        let pictures = root.join("pictures");
        let videos = pictures.join("videos");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&videos).unwrap();

        let result = validate_settings(&AppSettings {
            source_dir: source.to_string_lossy().to_string(),
            pictures_dir: Some(pictures.to_string_lossy().to_string()),
            videos_dir: Some(videos.to_string_lossy().to_string()),
            people_search_enabled: false,
        });
        assert!(result.unwrap_err().contains("cannot contain one another"));
        let _ = fs::remove_dir_all(root);
    }
}

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DecisionInfo {
    pub path: String,
    pub category: String,
    #[serde(default)]
    pub is_video: bool,
    pub folder_name: Option<String>,
    pub month_str: Option<String>,
    #[serde(default)]
    pub has_sidecar: bool,
    pub sidecar_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SidecarMove {
    pub src: String,
    pub dest: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerEntry {
    pub id: String,
    pub src: String,
    pub dest: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(default)]
    pub sidecars: Vec<SidecarMove>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LedgerFile {
    pub timestamp: String,
    pub action: String,
    pub total_success: usize,
    pub total_errors: usize,
    pub entries: Vec<LedgerEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub success: bool,
    pub moved: usize,
    pub errors: usize,
    pub ledger: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackResult {
    pub success: bool,
    pub restored_items: usize,
    pub restored_sidecars: usize,
    pub errors: usize,
    pub ledger_backup: Option<String>,
}

pub fn get_unique_destination_path(target_path: &Path) -> PathBuf {
    if !target_path.exists() {
        return target_path.to_path_buf();
    }

    let p_dir = match target_path.parent() {
        Some(p) => p,
        None => return target_path.to_path_buf(),
    };
    let name = match target_path.file_name().and_then(|n| n.to_str()) {
        Some(n) => n,
        None => return target_path.to_path_buf(),
    };

    let (base, ext) = if name.ends_with(".supplemental-metadata.json") {
        (&name[..name.len() - ".supplemental-metadata.json".len()], ".supplemental-metadata.json")
    } else {
        let stem = target_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        (stem, "")
    };

    let ext_str = if ext.is_empty() {
        let extension = target_path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if extension.is_empty() { String::new() } else { format!(".{}", extension) }
    } else {
        ext.to_string()
    };

    let mut counter = 1;
    let mut candidate = p_dir.join(format!("{}_{}{}", base, counter, ext_str));
    while candidate.exists() {
        counter += 1;
        candidate = p_dir.join(format!("{}_{}{}", base, counter, ext_str));
    }
    candidate
}

pub fn execute_triage_plan(
    source_dir: &Path,
    decisions: HashMap<String, DecisionInfo>,
    action: &str,
    pictures_dir: Option<PathBuf>,
    videos_dir: Option<PathBuf>,
) -> Result<ExecutionResult, String> {
    let source_dir = source_dir.canonicalize().map_err(|e| format!("Invalid path: {}", e))?;
    let pics_dir = resolve_destination(&source_dir, pictures_dir, "Pictures")?;
    let vids_dir = resolve_destination(&source_dir, videos_dir, "Videos")?;
    if pics_dir == vids_dir || pics_dir.starts_with(&vids_dir) || vids_dir.starts_with(&pics_dir) {
        return Err("Pictures and Videos destinations must be different non-overlapping directories".to_string());
    }
    let docs_dir = source_dir.join("Pictures_Doc");
    let trash_dir = source_dir.join("Trash");

    let mut ledger_entries = Vec::new();
    let mut success_count = 0;
    let mut error_count = 0;

    for (file_id, info) in decisions {
        let src_path = PathBuf::from(&info.path);
        if !src_path.exists() {
            continue;
        }

        let cat = info.category.to_uppercase();
        if cat == "SKIP" {
            ledger_entries.push(LedgerEntry {
                id: file_id,
                src: info.path.clone(),
                dest: info.path.clone(),
                category: Some("SKIP".to_string()),
                sidecars: Vec::new(),
            });
            continue;
        }

        let dest_folder = info.folder_name.unwrap_or_else(|| "Daily Life".to_string());
        let month_str = info.month_str.unwrap_or_else(|| "General".to_string());

        let target_dir = match cat.as_str() {
            "TRASH" => trash_dir.join(&month_str),
            "DOCUMENT" => docs_dir.join(&month_str),
            _ => {
                if info.is_video {
                    vids_dir.join(&dest_folder)
                } else {
                    pics_dir.join(&dest_folder)
                }
            }
        };

        if fs::create_dir_all(&target_dir).is_err() {
            error_count += 1;
            continue;
        }

        let filename = match src_path.file_name() {
            Some(f) => f,
            None => continue,
        };

        let dest_file = get_unique_destination_path(&target_dir.join(filename));

        // 1. Move or Copy main file
        let op_res = if action == "move" {
            fs::rename(&src_path, &dest_file).or_else(|_| {
                fs::copy(&src_path, &dest_file).and_then(|_| fs::remove_file(&src_path))
            })
        } else {
            fs::copy(&src_path, &dest_file).map(|_| ())
        };

        if let Err(_) = op_res {
            error_count += 1;
            continue;
        }

        let mut item_ledger = LedgerEntry {
            id: file_id,
            src: src_path.to_string_lossy().to_string(),
            dest: dest_file.to_string_lossy().to_string(),
            category: Some(cat),
            sidecars: Vec::new(),
        };

        // 2. Locate and relocate sidecars
        let mut candidates = Vec::new();
        if let Some(ref sc) = info.sidecar_path {
            let sc_p = PathBuf::from(sc);
            if sc_p.exists() {
                candidates.push(sc_p);
            }
        } else if let Some(p_dir) = src_path.parent() {
            let name = src_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
            let stem = src_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            candidates.push(p_dir.join(format!("{}.supplemental-metadata.json", name)));
            candidates.push(p_dir.join(format!("{}.json", name)));
            candidates.push(p_dir.join(format!("{}.supplemental-metadata.json", stem)));
            candidates.push(p_dir.join(format!("{}.json", stem)));
        }

        for sc in candidates {
            if sc.exists() && sc.is_file() {
                let sc_name = sc.file_name().and_then(|n| n.to_str()).unwrap_or("");
                let src_name = src_path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                let dest_name = dest_file.file_name().and_then(|n| n.to_str()).unwrap_or("");

                let target_sc_name = if dest_name != src_name {
                    if sc_name.ends_with(".supplemental-metadata.json") {
                        format!("{}.supplemental-metadata.json", dest_name)
                    } else if sc_name.ends_with(".json") {
                        format!("{}.json", dest_name)
                    } else {
                        sc_name.to_string()
                    }
                } else {
                    sc_name.to_string()
                };

                let dest_sc = get_unique_destination_path(&target_dir.join(target_sc_name));

                let sc_op = if action == "move" {
                    fs::rename(&sc, &dest_sc).or_else(|_| {
                        fs::copy(&sc, &dest_sc).and_then(|_| fs::remove_file(&sc))
                    })
                } else {
                    fs::copy(&sc, &dest_sc).map(|_| ())
                };

                if sc_op.is_ok() {
                    item_ledger.sidecars.push(SidecarMove {
                        src: sc.to_string_lossy().to_string(),
                        dest: dest_sc.to_string_lossy().to_string(),
                    });
                }
            }
        }

        ledger_entries.push(item_ledger);
        success_count += 1;
    }

    let ledger_file = source_dir.join("_dreamcatcher_ledger.json");
    let ledger_data = LedgerFile {
        timestamp: chrono::Utc::now().to_rfc3339(),
        action: action.to_string(),
        total_success: success_count,
        total_errors: error_count,
        entries: ledger_entries,
    };

    if let Ok(json_bytes) = serde_json::to_string_pretty(&ledger_data) {
        let _ = fs::write(&ledger_file, json_bytes);
    }

    Ok(ExecutionResult {
        success: true,
        moved: success_count,
        errors: error_count,
        ledger: ledger_file.to_string_lossy().to_string(),
    })
}

fn resolve_destination(source_dir: &Path, configured: Option<PathBuf>, label: &str) -> Result<PathBuf, String> {
    let is_configured = configured.is_some();
    let destination = configured.unwrap_or_else(|| source_dir.join(label));
    if !destination.exists() {
        if is_configured {
            return Err(format!("{} destination does not exist: {:?}", label, destination));
        }
        return Ok(destination);
    }
    if !destination.is_dir() {
        return Err(format!("{} destination is not a directory: {:?}", label, destination));
    }
    let canonical = destination.canonicalize().map_err(|e| format!("Invalid {} destination: {}", label, e))?;
    if canonical == source_dir {
        return Err(format!("{} destination cannot be the source folder", label));
    }
    Ok(canonical)
}

pub fn rollback_triage_plan(source_dir: &Path) -> Result<RollbackResult, String> {
    let source_dir = source_dir.canonicalize().map_err(|e| format!("Invalid path: {}", e))?;
    let ledger_file = source_dir.join("_dreamcatcher_ledger.json");
    if !ledger_file.exists() {
        return Err(format!("No triage ledger found at {:?}", ledger_file));
    }

    let content = fs::read_to_string(&ledger_file).map_err(|e| format!("Cannot read ledger: {}", e))?;
    let ledger_data: LedgerFile = serde_json::from_str(&content).map_err(|e| format!("Invalid ledger JSON: {}", e))?;

    let action = ledger_data.action;
    let mut restored_items = 0;
    let mut restored_sidecars = 0;
    let mut errors = 0;

    for item in ledger_data.entries {
        if item.category.as_deref() == Some("SKIP") || item.dest == item.src {
            continue;
        }

        let dest_path = PathBuf::from(&item.dest);
        let src_path = PathBuf::from(&item.src);

        if dest_path.exists() {
            if let Some(parent) = src_path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            let res = if action == "move" {
                fs::rename(&dest_path, &src_path).or_else(|_| {
                    fs::copy(&dest_path, &src_path).and_then(|_| fs::remove_file(&dest_path))
                })
            } else {
                fs::remove_file(&dest_path)
            };

            if res.is_ok() {
                restored_items += 1;
            } else {
                errors += 1;
            }
        }

        for sc in item.sidecars {
            let sc_dest = PathBuf::from(&sc.dest);
            let sc_src = PathBuf::from(&sc.src);
            if sc_dest.exists() {
                if let Some(parent) = sc_src.parent() {
                    let _ = fs::create_dir_all(parent);
                }
                let res = if action == "move" {
                    fs::rename(&sc_dest, &sc_src).or_else(|_| {
                        fs::copy(&sc_dest, &sc_src).and_then(|_| fs::remove_file(&sc_dest))
                    })
                } else {
                    fs::remove_file(&sc_dest)
                };

                if res.is_ok() {
                    restored_sidecars += 1;
                } else {
                    errors += 1;
                }
            }
        }
    }

    // Clean up empty directories under Pictures, Videos, Pictures_Doc, Trash
    for sub in &["Pictures", "Videos", "Pictures_Doc", "Trash"] {
        let target_sub = source_dir.join(sub);
        if target_sub.exists() && target_sub.is_dir() {
            for entry in WalkDir::new(&target_sub).contents_first(true).into_iter().filter_map(|e| e.ok()) {
                if entry.file_type().is_dir() {
                    let _ = fs::remove_dir(entry.path());
                }
            }
            let _ = fs::remove_dir(&target_sub);
        }
    }

    let bak_ledger = ledger_file.with_extension("json.bak");
    let _ = fs::rename(&ledger_file, &bak_ledger);

    Ok(RollbackResult {
        success: true,
        restored_items,
        restored_sidecars,
        errors,
        ledger_backup: Some(bak_ledger.to_string_lossy().to_string()),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn sha256_file(path: &Path) -> String {
        let bytes = fs::read(path).unwrap();
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        format!("{:x}", hasher.finalize())
    }

    #[test]
    fn test_collision_handling_and_rollback_restores_media_and_sidecars() {
        let root = std::env::temp_dir().join(format!("dc-executor-test-{}", std::process::id()));
        let source = root.join("source");
        let pictures = root.join("pictures");
        let videos = root.join("videos");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&pictures).unwrap();
        fs::create_dir_all(&videos).unwrap();

        // Create pre-existing file in pictures/Daily Life with name photo.jpg to trigger collision
        let existing_dest_dir = pictures.join("Daily Life");
        fs::create_dir_all(&existing_dest_dir).unwrap();
        let existing_dest_file = existing_dest_dir.join("photo.jpg");
        fs::write(&existing_dest_file, b"EXISTING PRE-COLLISION FILE CONTENT").unwrap();
        let existing_hash_before = sha256_file(&existing_dest_file);

        // Also create pre-existing sidecar in destination
        let existing_dest_sidecar = existing_dest_dir.join("photo.jpg.supplemental-metadata.json");
        fs::write(&existing_dest_sidecar, b"EXISTING PRE-COLLISION SIDECAR CONTENT").unwrap();
        let existing_sidecar_hash_before = sha256_file(&existing_dest_sidecar);

        // Create source photo and sidecar
        let src_photo = source.join("photo.jpg");
        fs::write(&src_photo, b"NEW SOURCE PHOTO CONTENT").unwrap();
        let src_photo_hash = sha256_file(&src_photo);

        let src_sidecar = source.join("photo.jpg.supplemental-metadata.json");
        fs::write(&src_sidecar, b"SOURCE SIDECAR JSON CONTENT").unwrap();
        let src_sidecar_hash = sha256_file(&src_sidecar);

        // Create source video
        let src_video = source.join("clip.mp4");
        fs::write(&src_video, b"SOURCE VIDEO CONTENT").unwrap();
        let src_video_hash = sha256_file(&src_video);

        let mut decisions = HashMap::new();
        decisions.insert("photo-1".to_string(), DecisionInfo {
            path: src_photo.to_string_lossy().to_string(),
            category: "PHOTO".to_string(),
            is_video: false,
            folder_name: Some("Daily Life".to_string()),
            month_str: Some("2024-01".to_string()),
            has_sidecar: true,
            sidecar_path: Some(src_sidecar.to_string_lossy().to_string()),
        });
        decisions.insert("video-1".to_string(), DecisionInfo {
            path: src_video.to_string_lossy().to_string(),
            category: "VIDEO".to_string(),
            is_video: true,
            folder_name: Some("Vacation".to_string()),
            month_str: Some("2024-01".to_string()),
            has_sidecar: false,
            sidecar_path: None,
        });

        // Execute triage with move action
        let exec_result = execute_triage_plan(
            &source,
            decisions,
            "move",
            Some(pictures.clone()),
            Some(videos.clone()),
        ).expect("execute triage plan failed");

        assert!(exec_result.success);
        assert_eq!(exec_result.moved, 2);
        assert_eq!(exec_result.errors, 0);

        // 1. Confirm pre-existing file and sidecar in destination were NOT overwritten
        assert_eq!(sha256_file(&existing_dest_file), existing_hash_before, "Existing destination file was overwritten!");
        assert_eq!(sha256_file(&existing_dest_sidecar), existing_sidecar_hash_before, "Existing destination sidecar was overwritten!");

        // 2. Confirm collision resolution generated photo_1.jpg and photo_1.jpg.supplemental-metadata.json
        let collided_photo = existing_dest_dir.join("photo_1.jpg");
        assert!(collided_photo.exists(), "Collided photo not created as photo_1.jpg");
        assert_eq!(sha256_file(&collided_photo), src_photo_hash);

        let collided_sidecar = existing_dest_dir.join("photo_1.jpg.supplemental-metadata.json");
        assert!(collided_sidecar.exists(), "Collided sidecar not renamed appropriately");
        assert_eq!(sha256_file(&collided_sidecar), src_sidecar_hash);

        // Confirm video was moved to videos/Vacation/clip.mp4
        let dest_video = videos.join("Vacation").join("clip.mp4");
        assert!(dest_video.exists(), "Destination video not found");
        assert_eq!(sha256_file(&dest_video), src_video_hash);

        // Source files should no longer be in source
        assert!(!src_photo.exists());
        assert!(!src_sidecar.exists());
        assert!(!src_video.exists());

        // 3. Rollback triage plan
        let rollback_result = rollback_triage_plan(&source).expect("rollback triage failed");
        assert!(rollback_result.success);
        assert_eq!(rollback_result.restored_items, 2);
        assert_eq!(rollback_result.restored_sidecars, 1);
        assert_eq!(rollback_result.errors, 0);

        // 4. Confirm source files and sidecars are restored with identical hashes
        assert!(src_photo.exists());
        assert_eq!(sha256_file(&src_photo), src_photo_hash);
        assert!(src_sidecar.exists());
        assert_eq!(sha256_file(&src_sidecar), src_sidecar_hash);
        assert!(src_video.exists());
        assert_eq!(sha256_file(&src_video), src_video_hash);

        // 5. Confirm existing destination file is still intact and collided items are gone
        assert_eq!(sha256_file(&existing_dest_file), existing_hash_before);
        assert_eq!(sha256_file(&existing_dest_sidecar), existing_sidecar_hash_before);
        assert!(!collided_photo.exists());
        assert!(!collided_sidecar.exists());
        assert!(!dest_video.exists());

        let _ = fs::remove_dir_all(root);
    }
}

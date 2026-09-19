mod classifier;
mod clustering;
mod executor;
mod media;
mod scanner;

use classifier::{analyze_image, classify_heuristic, ClassificationResult};
use clustering::{cluster_items, Cluster};
use executor::{execute_triage_plan, rollback_triage_plan, DecisionInfo, ExecutionResult, RollbackResult};
use media::{read_full_media, render_thumbnail};
use scanner::{scan_directory, MediaItem};

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use tauri::http::{Response, StatusCode};

#[derive(Debug, Serialize, Deserialize)]
pub struct ScanResultPayload {
    pub source_dir: String,
    pub month_filter: Option<String>,
    pub total_discovered: usize,
    pub total_scanned: usize,
    pub obvious_docs: Vec<MediaItem>,
    pub obvious_photos: Vec<MediaItem>,
    pub mixed_items: Vec<MediaItem>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OllamaStatus {
    pub connected: bool,
    pub url: String,
    pub models: Vec<String>,
    pub has_moondream: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SystemStatusPayload {
    pub status: String,
    pub ollama: OllamaStatus,
}

async fn query_ollama(client: &reqwest::Client, url: &str) -> (bool, Vec<String>, bool) {
    if let Ok(resp) = client.get(format!("{}/api/tags", url)).send().await {
        if resp.status().is_success() {
            if let Ok(data) = resp.json::<serde_json::Value>().await {
                let mut models = Vec::new();
                let mut has_moondream = false;
                if let Some(arr) = data.get("models").and_then(|m| m.as_array()) {
                    for m in arr {
                        if let Some(name) = m.get("name").and_then(|n| n.as_str()) {
                            let n_str = name.to_string();
                            if n_str.to_lowercase().contains("moondream") {
                                has_moondream = true;
                            }
                            models.push(n_str);
                        }
                    }
                }
                return (true, models, has_moondream);
            }
        }
    }
    (false, Vec::new(), false)
}

#[tauri::command]
async fn get_system_status(ollama_url: Option<String>) -> Result<SystemStatusPayload, String> {
    let url = ollama_url.unwrap_or_else(|| "http://127.0.0.1:11434".to_string());
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
        .unwrap_or_default();

    let (mut connected, mut models, mut has_moondream) = query_ollama(&client, &url).await;

    if !connected && (url.contains("127.0.0.1") || url.contains("localhost")) {
        if let Ok(local_app_data) = std::env::var("LOCALAPPDATA") {
            let ollama_bin = PathBuf::from(local_app_data)
                .join("Programs")
                .join("Ollama")
                .join("ollama.exe");
            if ollama_bin.exists() {
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    const CREATE_NO_WINDOW: u32 = 0x08000000;
                    const DETACHED_PROCESS: u32 = 0x00000008;

                    let _ = std::process::Command::new(&ollama_bin)
                        .arg("serve")
                        .stdin(std::process::Stdio::null())
                        .stdout(std::process::Stdio::null())
                        .stderr(std::process::Stdio::null())
                        .creation_flags(CREATE_NO_WINDOW | DETACHED_PROCESS)
                        .spawn();
                }
                tokio::time::sleep(std::time::Duration::from_millis(1800)).await;
                let (recheck_conn, recheck_models, recheck_moon) = query_ollama(&client, &url).await;
                connected = recheck_conn;
                models = recheck_models;
                has_moondream = recheck_moon;
            }
        }
    }

    Ok(SystemStatusPayload {
        status: "ready".to_string(),
        ollama: OllamaStatus {
            connected,
            url,
            models,
            has_moondream,
        },
    })
}

#[tauri::command]
async fn scan_folder(
    source_dir: String,
    month_filter: Option<String>,
    run_ai_on_ambiguous: Option<bool>,
    backend: Option<String>,
    ollama_model: Option<String>,
    ollama_url: Option<String>,
) -> Result<ScanResultPayload, String> {
    let src = PathBuf::from(&source_dir);
    if !src.exists() || !src.is_dir() {
        return Err(format!("Directory does not exist: {}", source_dir));
    }

    let (raw_items, total_discovered) =
        scan_directory(&src, month_filter.as_deref())?;

    let run_ai = run_ai_on_ambiguous.unwrap_or(false);
    let be = backend.unwrap_or_else(|| "ollama".to_string());
    let model = ollama_model.unwrap_or_else(|| "moondream".to_string());
    let o_url = ollama_url.unwrap_or_else(|| "http://127.0.0.1:11434".to_string());

    let client = reqwest::Client::new();
    let mut obvious_docs = Vec::new();
    let mut obvious_photos = Vec::new();
    let mut mixed_items = Vec::new();

    for mut it in raw_items {
        let p = PathBuf::from(&it.path);
        let analysis = if run_ai {
            analyze_image(&client, &p, &be, &model, &o_url).await
        } else {
            classify_heuristic(&p)
        };

        it.category = analysis.category;
        it.tier = analysis.tier;
        it.reason = analysis.reason;
        it.caption = analysis.caption;
        it.is_cached = Some(analysis.is_cached);

        if it.tier == "OBVIOUS" && it.category == "DOCUMENT" {
            obvious_docs.push(it);
        } else if it.tier == "OBVIOUS" && it.category == "PHOTO" {
            obvious_photos.push(it);
        } else {
            mixed_items.push(it);
        }
    }

    let total_scanned = obvious_docs.len() + obvious_photos.len() + mixed_items.len();

    Ok(ScanResultPayload {
        source_dir,
        month_filter,
        total_discovered,
        total_scanned,
        obvious_docs,
        obvious_photos,
        mixed_items,
    })
}

#[tauri::command]
async fn classify_single(
    path: String,
    backend: Option<String>,
    ollama_model: Option<String>,
    ollama_url: Option<String>,
) -> Result<ClassificationResult, String> {
    let p = PathBuf::from(&path);
    if !p.exists() {
        return Err("File not found".to_string());
    }

    let be = backend.unwrap_or_else(|| "ollama".to_string());
    let model = ollama_model.unwrap_or_else(|| "moondream".to_string());
    let url = ollama_url.unwrap_or_else(|| "http://127.0.0.1:11434".to_string());
    let client = reqwest::Client::new();

    Ok(analyze_image(&client, &p, &be, &model, &url).await)
}

#[tauri::command]
async fn classify_batch(
    paths: Vec<String>,
    backend: Option<String>,
    ollama_model: Option<String>,
    ollama_url: Option<String>,
) -> Result<Vec<ClassificationResultWithItem>, String> {
    let be = backend.unwrap_or_else(|| "ollama".to_string());
    let model = ollama_model.unwrap_or_else(|| "moondream".to_string());
    let url = ollama_url.unwrap_or_else(|| "http://127.0.0.1:11434".to_string());
    let client = reqwest::Client::new();

    let mut results = Vec::new();
    for path_str in paths {
        let p = PathBuf::from(&path_str);
        if !p.exists() {
            continue;
        }
        let res = analyze_image(&client, &p, &be, &model, &url).await;
        results.push(ClassificationResultWithItem {
            path: path_str,
            category: res.category,
            tier: res.tier,
            reason: res.reason,
            caption: res.caption,
        });
    }
    Ok(results)
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ClassificationResultWithItem {
    pub path: String,
    pub category: String,
    pub tier: String,
    pub reason: String,
    pub caption: String,
}

#[tauri::command]
async fn cluster_media(
    items: Vec<MediaItem>,
    cluster_hours: Option<f64>,
    min_cluster_size: Option<usize>,
) -> Result<Vec<Cluster>, String> {
    let client = reqwest::Client::new();
    let hours = cluster_hours.unwrap_or(4.0);
    let min_size = min_cluster_size.unwrap_or(5);
    Ok(cluster_items(&client, items, hours, min_size).await)
}

#[tauri::command]
async fn execute_triage(
    source_dir: String,
    decisions: HashMap<String, DecisionInfo>,
    action: Option<String>,
) -> Result<ExecutionResult, String> {
    let src = PathBuf::from(&source_dir);
    let act = action.unwrap_or_else(|| "move".to_string());
    execute_triage_plan(&src, decisions, &act)
}

#[tauri::command]
async fn rollback_triage(source_dir: String) -> Result<RollbackResult, String> {
    let src = PathBuf::from(&source_dir);
    rollback_triage_plan(&src)
}

fn url_decode(input: &str) -> String {
    percent_encoding::percent_decode_str(input)
        .decode_utf8_lossy()
        .to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .register_uri_scheme_protocol("dc-media", |_app, request| {
            let uri = request.uri().to_string();
            // uri format: dc-media://thumbnail?path=...&max_dim=320 or dc-media://localhost/thumbnail?path=...
            let query = match request.uri().query() {
                Some(q) => q,
                None => {
                    return Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .body(Vec::new())
                        .unwrap();
                }
            };

            let mut params: HashMap<String, String> = HashMap::new();
            for part in query.split('&') {
                let mut kv = part.splitn(2, '=');
                if let (Some(k), Some(v)) = (kv.next(), kv.next()) {
                    params.insert(url_decode(k), url_decode(v));
                }
            }

            let path_str = match params.get("path") {
                Some(p) => p,
                None => {
                    return Response::builder()
                        .status(StatusCode::BAD_REQUEST)
                        .body(Vec::new())
                        .unwrap();
                }
            };

            let filepath = PathBuf::from(path_str);
            if !filepath.exists() {
                return Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Vec::new())
                    .unwrap();
            }

            let is_full = uri.contains("/full") || uri.contains("full?");
            if is_full {
                match read_full_media(&filepath) {
                    Ok((bytes, mime)) => Response::builder()
                        .header("Content-Type", mime)
                        .header("Cache-Control", "public, max-age=86400")
                        .body(bytes)
                        .unwrap(),
                    Err(_) => Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Vec::new())
                        .unwrap(),
                }
            } else {
                let max_dim = params
                    .get("max_dim")
                    .and_then(|d| d.parse::<u32>().ok())
                    .unwrap_or(320);

                match render_thumbnail(&filepath, max_dim) {
                    Ok((bytes, mime)) => Response::builder()
                        .header("Content-Type", mime)
                        .header("Cache-Control", "public, max-age=86400")
                        .body(bytes)
                        .unwrap(),
                    Err(_) => Response::builder()
                        .status(StatusCode::INTERNAL_SERVER_ERROR)
                        .body(Vec::new())
                        .unwrap(),
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_system_status,
            scan_folder,
            classify_single,
            classify_batch,
            cluster_media,
            execute_triage,
            rollback_triage,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

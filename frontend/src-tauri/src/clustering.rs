use crate::scanner::MediaItem;
use chrono::{Datelike, NaiveDate, Weekday};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Cluster {
    pub id: String,
    pub date_str: String,
    pub folder_name: String,
    pub is_holiday: bool,
    pub is_event: bool,
    pub items: Vec<MediaItem>,
}

static GEO_CACHE: Mutex<Option<HashMap<(i32, i32), Option<String>>>> = Mutex::new(None);
const LOCAL_CITIES: &[&str] = &["fargo", "west fargo", "moorhead", "dilworth", "horace", "harwood"];

pub fn get_holiday_name(d: NaiveDate) -> Option<&'static str> {
    let (year, month, day) = (d.year(), d.month(), d.day());

    match (month, day) {
        (1, 1) => return Some("New Year's Day"),
        (2, 14) => return Some("Valentine's Day"),
        (3, 17) => return Some("St. Patrick's Day"),
        (7, 4) => return Some("July 4th"),
        (10, 31) => return Some("Halloween"),
        (11, 11) => return Some("Veterans Day"),
        (12, 24) => return Some("Christmas Eve"),
        (12, 25) => return Some("Christmas"),
        (12, 31) => return Some("New Year's Eve"),
        _ => {}
    }

    // Easter algorithm
    let a = year % 19;
    let b = year / 100;
    let c = year % 100;
    let d_val = b / 4;
    let e = b % 4;
    let f = (b + 8) / 25;
    let g = (b - f + 1) / 3;
    let h = (19 * a + b - d_val - g + 15) % 30;
    let i = c / 4;
    let k = c % 4;
    let l = (32 + 2 * e + 2 * i - h - k) % 7;
    let m = (a + 11 * h + 22 * l) / 451;
    let easter_month = ((h + l - 7 * m + 114) / 31) as u32;
    let easter_day = (((h + l - 7 * m + 114) % 31) + 1) as u32;

    if month == easter_month && day == easter_day {
        return Some("Easter");
    }

    // Thanksgiving (4th Thursday in November)
    if month == 11 && d.weekday() == Weekday::Thu && (22..=28).contains(&day) {
        return Some("Thanksgiving");
    }

    None
}

pub async fn reverse_geocode_city(
    client: &reqwest::Client,
    lat: f64,
    lon: f64,
) -> Option<String> {
    let key = ((lat * 1000.0).round() as i32, (lon * 1000.0).round() as i32);

    {
        let mut lock = GEO_CACHE.lock().unwrap();
        if lock.is_none() {
            *lock = Some(HashMap::new());
        }
        if let Some(cached) = lock.as_ref().unwrap().get(&key) {
            return cached.clone();
        }
    }

    let url = "https://nominatim.openstreetmap.org/reverse";
    let res = client
        .get(url)
        .header("User-Agent", "DreamCatcher-PhotoTriage-Tauri/1.0")
        .query(&[
            ("lat", lat.to_string()),
            ("lon", lon.to_string()),
            ("format", "json".to_string()),
            ("zoom", "12".to_string()),
            ("addressdetails", "1".to_string()),
        ])
        .send()
        .await;

    let mut city = None;
    if let Ok(resp) = res {
        if resp.status().is_success() {
            if let Ok(json) = resp.json::<serde_json::Value>().await {
                if let Some(addr) = json.get("address") {
                    let c = addr
                        .get("city")
                        .or_else(|| addr.get("town"))
                        .or_else(|| addr.get("village"))
                        .or_else(|| addr.get("municipality"))
                        .and_then(|v| v.as_str());
                    if let Some(c_str) = c {
                        city = Some(c_str.to_string());
                    }
                }
            }
        }
    }

    let mut lock = GEO_CACHE.lock().unwrap();
    if let Some(map) = lock.as_mut() {
        map.insert(key, city.clone());
    }

    city
}

pub async fn cluster_items(
    client: &reqwest::Client,
    items: Vec<MediaItem>,
    cluster_hours: f64,
    min_cluster_size: usize,
) -> Vec<Cluster> {
    if items.is_empty() {
        return Vec::new();
    }

    // Group items by calendar date
    let mut by_date: HashMap<String, Vec<MediaItem>> = HashMap::new();
    for it in items {
        by_date.entry(it.date_str.clone()).or_default().push(it);
    }

    let mut dates: Vec<String> = by_date.keys().cloned().collect();
    dates.sort();

    let mut clusters_out = Vec::new();

    for d_str in dates {
        let day_items = by_date.remove(&d_str).unwrap();
        let cal_date = match NaiveDate::parse_from_str(&d_str, "%Y-%m-%d") {
            Ok(d) => d,
            Err(_) => continue,
        };

        if let Some(holiday) = get_holiday_name(cal_date) {
            clusters_out.push(Cluster {
                id: format!("cluster_{}_holiday", d_str),
                date_str: d_str.clone(),
                folder_name: format!("({}) {}", d_str, holiday),
                is_holiday: true,
                is_event: true,
                items: day_items,
            });
            continue;
        }

        // Sub-cluster by temporal gaps
        let mut sub_clusters: Vec<Vec<MediaItem>> = Vec::new();
        let mut current_chunk: Vec<MediaItem> = Vec::new();

        for it in day_items {
            if current_chunk.is_empty() {
                current_chunk.push(it);
            } else {
                let prev_dt = chrono::DateTime::parse_from_rfc3339(&current_chunk.last().unwrap().timestamp).ok();
                let curr_dt = chrono::DateTime::parse_from_rfc3339(&it.timestamp).ok();
                let diff_hours = match (prev_dt, curr_dt) {
                    (Some(p), Some(c)) => (c.signed_duration_since(p).num_seconds() as f64) / 3600.0,
                    _ => 0.0,
                };

                if diff_hours <= cluster_hours {
                    current_chunk.push(it);
                } else {
                    sub_clusters.push(current_chunk);
                    current_chunk = vec![it];
                }
            }
        }
        if !current_chunk.is_empty() {
            sub_clusters.push(current_chunk);
        }

        let mut event_groups = Vec::new();
        let mut loose_items = Vec::new();

        for sc in sub_clusters {
            if sc.len() >= min_cluster_size {
                event_groups.push(sc);
            } else {
                loose_items.extend(sc);
            }
        }

        for (idx, eg) in event_groups.into_iter().enumerate() {
            let mut city = None;
            for it in &eg {
                if let Some((lat, lon)) = it.gps {
                    city = reverse_geocode_city(client, lat, lon).await;
                    if city.is_some() {
                        break;
                    }
                }
            }

            let label = match city {
                Some(ref c) if !LOCAL_CITIES.contains(&c.to_lowercase().as_str()) => {
                    format!("{} - Event", c)
                }
                _ => "Event Candidate".to_string(),
            };

            clusters_out.push(Cluster {
                id: format!("cluster_{}_{}", d_str, idx),
                date_str: d_str.clone(),
                folder_name: format!("({}) {}", d_str, label),
                is_holiday: false,
                is_event: true,
                items: eg,
            });
        }

        if !loose_items.is_empty() {
            clusters_out.push(Cluster {
                id: format!("cluster_{}_daily", d_str),
                date_str: d_str.clone(),
                folder_name: format!("({}) Daily Life", d_str),
                is_holiday: false,
                is_event: false,
                items: loose_items,
            });
        }
    }

    clusters_out
}

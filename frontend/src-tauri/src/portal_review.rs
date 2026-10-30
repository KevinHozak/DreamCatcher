//! Native-owned per-action review. Selected media is never read; the registered
//! Portal host alone owns credentials, final native consent and provider dispatch.
use crate::portal_host::{self, Receipt, Request};
use base64::{engine::general_purpose::STANDARD, Engine};
use portal_client::image::ImageReview;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::{Duration, Instant};
use tauri::{Manager, State, WebviewWindow};

const MODEL: &str = "google/gemini-3.5-flash-lite";
const ENDPOINT: &str = "google-vertex/us";
const POLICY: &str = "s010-us-context-v1-disabled-until-operator-approval";
#[cfg(test)]
const TEXT: &str = "Classify these generated colored shapes.";
#[derive(Clone, Copy, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Source {
    Generated,
    Selection,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PrepareArgs {
    pub source: Source,
    pub text: String,
    pub redact: bool,
}
#[derive(Serialize)]
pub struct Preview {
    session_id: String,
    fingerprint: String,
    app_id: &'static str,
    action: &'static str,
    model: &'static str,
    endpoint: &'static str,
    policy: &'static str,
    image: Option<String>,
    text: String,
    max_tokens: u32,
    redact: bool,
    can_send: bool,
    can_confirm_offline: bool,
    message: &'static str,
}
struct Pending {
    owner: String,
    id: String,
    fingerprint: String,
    expires: Instant,
    review: Option<ImageReview>,
    request: Option<Request>,
}
#[derive(Default)]
pub struct Reviews(
    Mutex<Option<Pending>>,
    Mutex<Option<(String, String, Arc<AtomicBool>)>>,
    bool,
);
fn error() -> String {
    "IMAGE_CAPABILITY_UNQUALIFIED".into()
}
fn nonce() -> Result<String, String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| "REVIEW_UNAVAILABLE".to_string())?;
    Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
impl Reviews {
    pub fn new(qualification: bool) -> Self {
        Self(Mutex::new(None), Mutex::new(None), qualification)
    }
    pub fn prepare(&self, owner: &str, args: PrepareArgs) -> Result<Preview, String> {
        if owner != "main" {
            return Err(error());
        }
        if self.1.lock().map_err(|_| error())?.is_some() {
            return Err("REMOTE_REVIEW_BUSY".into());
        }
        let mut state = self.0.lock().map_err(|_| error())?;
        *state = None; // Invalid preparation also revokes the older preview.
        if args.text.len() > 1024 {
            return Err("IMAGE_TEXT_TOO_LARGE".into());
        }
        let id = nonce()?;
        // Selection is denied before any filesystem, decoder, cache or network access.
        let mut pixel_sha256 = String::new();
        let review = if args.source == Source::Generated {
            let mut rgb = vec![0u8; 32 * 32 * 3];
            for y in 0..32 {
                for x in 0..32 {
                    let color = match (x < 16, y < 16) {
                        (true, true) => [255, 0, 0],
                        (false, true) => [0, 255, 0],
                        (true, false) => [0, 0, 255],
                        _ => [255, 255, 255],
                    };
                    rgb[(y * 32 + x) * 3..(y * 32 + x) * 3 + 3].copy_from_slice(&color);
                }
            }
            let masks = if args.redact {
                vec![(0, 0, 8, 8)]
            } else {
                vec![]
            };
            let mut reviewed = rgb.clone();
            if args.redact {
                for y in 0..8 {
                    reviewed[y * 32 * 3..y * 32 * 3 + 8 * 3].fill(0);
                }
            }
            pixel_sha256 = format!("{:x}", Sha256::digest(&reviewed));
            Some(
                ImageReview::new(
                    &id,
                    32,
                    32,
                    &rgb,
                    &args.text,
                    "dreamcatcher-vision",
                    64,
                    &masks,
                )
                .map_err(|_| "IMAGE_PREPARATION_INVALID".to_string())?,
            )
        } else {
            None
        };
        let fingerprint = review
            .as_ref()
            .map(|r| r.fingerprint().to_string())
            .unwrap_or_else(|| id.clone());
        let image = review
            .as_ref()
            .map(|r| format!("data:image/png;base64,{}", STANDARD.encode(r.png())));
        let generated = review.is_some();
        let can_send = generated && !self.2 && portal_host::available();
        let request = generated.then(|| Request {
            contract: portal_host::CONTRACT,
            operation: id.clone(),
            text: args.text.clone(),
            redact: args.redact,
            pixel_sha256,
        });
        *state = Some(Pending {
            owner: owner.into(),
            id: id.clone(),
            fingerprint: fingerprint.clone(),
            expires: Instant::now() + Duration::from_secs(60),
            review,
            request,
        });
        Ok(Preview {
            session_id: id,
            fingerprint,
            app_id: "dreamcatcher",
            action: "classify-image",
            model: MODEL,
            endpoint: ENDPOINT,
            policy: POLICY,
            image,
            text: if generated { args.text } else { String::new() },
            max_tokens: 64,
            redact: args.redact,
            can_send,
            can_confirm_offline: generated,
            message: if can_send {
                "Portal will open a native consent window before sending."
            } else if generated {
                "Generated example only. Confirmation sends nothing."
            } else {
                "Remote vision is unavailable. No selected media was read or sent."
            },
        })
    }
    pub fn finish(
        &self,
        owner: &str,
        id: &str,
        fingerprint: &str,
        send: bool,
    ) -> Result<&'static str, String> {
        let mut state = self.0.lock().map_err(|_| error())?;
        let p = state.as_ref().ok_or_else(|| "REVIEW_STALE".to_string())?;
        if p.owner != owner || p.id != id {
            return Err("REVIEW_STALE".into());
        }
        let mut p = state.take().unwrap();
        if p.expires <= Instant::now() || p.fingerprint != fingerprint {
            return Err("REVIEW_STALE".into());
        }
        // A UI boolean, fingerprint or nonce cannot grant production admission.
        if send {
            return Err(error());
        }
        let mut review = p.review.take().ok_or_else(error)?;
        let _approved = review
            .approve(fingerprint)
            .map_err(|_| "REVIEW_STALE".to_string())?;
        Ok("offline_reviewed")
    }
    pub fn send(&self, owner: &str, id: &str, fingerprint: &str) -> Result<Receipt, String> {
        self.send_using(owner, id, fingerprint, portal_host::launch)
    }
    fn send_using<F>(
        &self,
        owner: &str,
        id: &str,
        fingerprint: &str,
        launch: F,
    ) -> Result<Receipt, String>
    where
        F: FnOnce(Request, Arc<AtomicBool>) -> Receipt,
    {
        if self.2 {
            return Err(error());
        }
        let mut state = self.0.lock().map_err(|_| error())?;
        let p = state.as_ref().ok_or_else(|| "REVIEW_STALE".to_string())?;
        if p.owner != owner || owner != "main" || p.id != id {
            return Err("REVIEW_STALE".into());
        }
        let mut job = self.1.lock().map_err(|_| error())?;
        if job.is_some() {
            return Err("REMOTE_REVIEW_BUSY".into());
        }
        let mut p = state.take().unwrap();
        if p.expires <= Instant::now() || p.fingerprint != fingerprint {
            return Err("REVIEW_STALE".into());
        }
        let mut review = p.review.take().ok_or_else(error)?;
        let _approved = review
            .approve(fingerprint)
            .map_err(|_| "REVIEW_STALE".to_string())?;
        let request = p.request.take().ok_or_else(error)?;
        let cancel = Arc::new(AtomicBool::new(false));
        *job = Some((owner.into(), id.into(), cancel.clone()));
        drop(job);
        drop(state);
        let receipt = launch(request, cancel);
        if let Ok(mut job) = self.1.lock() {
            *job = None;
        }
        Ok(receipt)
    }
    pub fn cancel(&self, owner: &str, id: Option<&str>) {
        if let Ok(mut state) = self.0.lock() {
            if state
                .as_ref()
                .is_some_and(|p| p.owner == owner && id.is_none_or(|id| p.id == id))
            {
                *state = None;
            }
        }
        if let Ok(job) = self.1.lock() {
            if let Some((who, op, cancel)) = job.as_ref() {
                if who == owner && id.is_none_or(|id| id == op) {
                    cancel.store(true, Ordering::Release);
                }
            }
        }
    }
}
fn owner(window: &WebviewWindow) -> Result<String, String> {
    let url = window.url().map_err(|_| error())?;
    let trusted = (url.scheme() == "tauri" && url.host_str() == Some("localhost"))
        || (matches!(url.scheme(), "http" | "https") && url.host_str() == Some("tauri.localhost"))
        || (cfg!(debug_assertions)
            && url.scheme() == "http"
            && url.host_str() == Some("localhost")
            && url.port() == Some(5180));
    if window.label() != "main" || !trusted {
        return Err(error());
    }
    Ok(window.label().into())
}
#[tauri::command]
pub fn portal_prepare_review(
    window: WebviewWindow,
    state: State<'_, Reviews>,
    args: PrepareArgs,
) -> Result<Preview, String> {
    state.prepare(&owner(&window)?, args)
}
#[tauri::command]
pub fn portal_finish_review(
    window: WebviewWindow,
    state: State<'_, Reviews>,
    session_id: String,
    fingerprint: String,
    send: bool,
) -> Result<&'static str, String> {
    state.finish(&owner(&window)?, &session_id, &fingerprint, send)
}
#[tauri::command]
pub async fn portal_send_review(
    window: WebviewWindow,
    state: State<'_, Reviews>,
    session_id: String,
    fingerprint: String,
) -> Result<Receipt, String> {
    let who = owner(&window)?;
    let app = window.app_handle().clone();
    drop(state);
    tauri::async_runtime::spawn_blocking(move || {
        app.state::<Reviews>().send(&who, &session_id, &fingerprint)
    })
    .await
    .map_err(|_| "NATIVE_HOST_UNRESOLVED".to_string())?
}
#[tauri::command]
pub fn portal_cancel_review(
    window: WebviewWindow,
    state: State<'_, Reviews>,
    session_id: String,
) -> Result<(), String> {
    state.cancel(&owner(&window)?, Some(&session_id));
    Ok(())
}
pub fn close_window(window: &tauri::Window, event: &tauri::WindowEvent) {
    if matches!(event, tauri::WindowEvent::Destroyed) {
        window.state::<Reviews>().cancel(window.label(), None);
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn args(source: Source) -> PrepareArgs {
        PrepareArgs {
            source,
            text: TEXT.into(),
            redact: true,
        }
    }
    #[test]
    fn exact_generated_preview_and_single_use() {
        let s = Reviews::default();
        let p = s.prepare("main", args(Source::Generated)).unwrap();
        let png = STANDARD
            .decode(p.image.as_ref().unwrap().split(',').nth(1).unwrap())
            .unwrap();
        let decoded = image::load_from_memory(&png).unwrap().to_rgb8();
        assert_eq!(decoded.dimensions(), (32, 32));
        assert_eq!(decoded.get_pixel(0, 0).0, [0, 0, 0]);
        assert_eq!(decoded.get_pixel(16, 0).0, [0, 255, 0]);
        assert!(!p.can_send);
        assert!(p.can_confirm_offline);
        assert_eq!(
            s.finish("main", &p.session_id, &p.fingerprint, false)
                .unwrap(),
            "offline_reviewed"
        );
        assert!(s
            .finish("main", &p.session_id, &p.fingerprint, false)
            .is_err());
    }
    #[test]
    fn selected_media_and_forged_send_never_admit() {
        for source in [Source::Selection, Source::Generated] {
            let s = Reviews::default();
            let p = s.prepare("main", args(source)).unwrap();
            if source == Source::Selection {
                assert!(p.image.is_none());
                assert!(p.text.is_empty());
            }
            assert_eq!(
                s.finish("main", &p.session_id, &p.fingerprint, true)
                    .unwrap_err(),
                "IMAGE_CAPABILITY_UNQUALIFIED"
            );
            assert!(s
                .finish("main", &p.session_id, &p.fingerprint, false)
                .is_err());
        }
    }
    #[test]
    fn stale_wrong_window_cancel_and_expiry() {
        let s = Reviews::default();
        let old = s.prepare("main", args(Source::Generated)).unwrap();
        let p = s.prepare("main", args(Source::Generated)).unwrap();
        assert!(s
            .finish("main", &old.session_id, &old.fingerprint, false)
            .is_err());
        assert!(s
            .finish("other", &p.session_id, &p.fingerprint, false)
            .is_err());
        s.cancel("main", Some(&old.session_id));
        s.0.lock().unwrap().as_mut().unwrap().expires = Instant::now();
        assert!(s
            .finish("main", &p.session_id, &p.fingerprint, false)
            .is_err());
        let p = s.prepare("main", args(Source::Generated)).unwrap();
        s.cancel("main", None);
        assert!(s
            .finish("main", &p.session_id, &p.fingerprint, false)
            .is_err());
    }
    #[test]
    fn native_send_consumes_review_and_qualification_cannot_launch() {
        let state = Reviews::default();
        let p = state.prepare("main", args(Source::Generated)).unwrap();
        let result = state
            .send_using("main", &p.session_id, &p.fingerprint, |request, cancel| {
                assert_eq!(request.operation, p.session_id);
                assert_eq!(
                    request.pixel_sha256,
                    "0c3a237648b687d3fe12ccaf4b43ce34d4bb47fbe2e4f799ecacbc874eb78931"
                );
                assert!(!cancel.load(Ordering::Acquire));
                Receipt::local(&request.operation, "NATIVE_HOST_UNAVAILABLE", false)
            })
            .unwrap();
        assert_eq!(result.billing_state, "not_admitted");
        assert!(state.send("main", &p.session_id, &p.fingerprint).is_err());
        let state = Reviews::new(true);
        let p = state.prepare("main", args(Source::Generated)).unwrap();
        assert!(!p.can_send);
        assert!(state
            .send_using("main", &p.session_id, &p.fingerprint, |_, _| panic!(
                "qualification cannot launch"
            ))
            .is_err());
        let state = Reviews::default();
        let p = state.prepare("main", args(Source::Selection)).unwrap();
        assert!(state
            .send_using("main", &p.session_id, &p.fingerprint, |_, _| panic!(
                "selection cannot launch"
            ))
            .is_err());
    }
    #[test]
    fn cancelling_running_host_marks_it_and_prevents_overlap() {
        let state = Arc::new(Reviews::default());
        let p = state.prepare("main", args(Source::Generated)).unwrap();
        let id = p.session_id.clone();
        let fp = p.fingerprint.clone();
        let (tx, rx) = std::sync::mpsc::channel();
        let worker = state.clone();
        let thread = std::thread::spawn(move || {
            worker
                .send_using("main", &id, &fp, |request, cancel| {
                    tx.send(()).unwrap();
                    let deadline = Instant::now() + Duration::from_secs(3);
                    while !cancel.load(Ordering::Acquire) && Instant::now() < deadline {
                        std::thread::sleep(Duration::from_millis(10));
                    }
                    assert!(cancel.load(Ordering::Acquire));
                    Receipt::local(&request.operation, "NATIVE_HOST_CANCELLED", true)
                })
                .unwrap()
        });
        rx.recv_timeout(Duration::from_secs(2)).unwrap();
        assert!(state.prepare("main", args(Source::Generated)).is_err());
        state.cancel("main", Some(&p.session_id));
        assert_eq!(thread.join().unwrap().billing_state, "unknown");
    }
    #[test]
    fn invalid_input_and_changed_fingerprint_revoke() {
        let s = Reviews::default();
        let p = s.prepare("main", args(Source::Generated)).unwrap();
        assert!(s
            .prepare(
                "main",
                PrepareArgs {
                    source: Source::Generated,
                    text: "x".repeat(1025),
                    redact: false
                }
            )
            .is_err());
        assert!(s
            .finish("main", &p.session_id, &p.fingerprint, false)
            .is_err());
        let p = s.prepare("main", args(Source::Generated)).unwrap();
        assert!(s.finish("main", &p.session_id, "forged", false).is_err());
        assert!(s
            .finish("main", &p.session_id, &p.fingerprint, false)
            .is_err());
        assert!(serde_json::from_str::<PrepareArgs>(
            r#"{"source":"generated","text":"x","redact":true,"path":"private"}"#
        )
        .is_err());
    }
}

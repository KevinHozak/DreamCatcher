//! Registered native consent host. No keys, grants or provider calls in Tauri.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

pub const CONTRACT: &str = "portal.native-image/1";
pub const MODEL: &str = "google/gemini-3.5-flash-lite";
pub const ENDPOINT: &str = "google-vertex/us";
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Descriptor {
    contract: String,
    executable: String,
    executable_sha256: String,
    bootstrap: String,
    bootstrap_sha256: String,
    state: String,
}
#[derive(Serialize)]
pub struct Request {
    pub contract: &'static str,
    pub operation: String,
    pub text: String,
    pub redact: bool,
    pub pixel_sha256: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub contract: String,
    pub operation: String,
    pub status: String,
    pub billing_state: String,
    pub model: String,
    pub endpoint: String,
    pub cost_usd_micros: Option<u64>,
    pub tokens: Option<u64>,
    pub classification: Option<String>,
    pub code: Option<String>,
}
impl Receipt {
    pub fn local(operation: &str, code: &str, attempted: bool) -> Self {
        Self {
            contract: CONTRACT.into(),
            operation: operation.into(),
            status: "unavailable".into(),
            billing_state: if attempted { "unknown" } else { "not_admitted" }.into(),
            model: MODEL.into(),
            endpoint: ENDPOINT.into(),
            cost_usd_micros: None,
            tokens: None,
            classification: None,
            code: Some(code.into()),
        }
    }
    fn valid(&self, operation: &str) -> bool {
        self.contract == CONTRACT
            && self.operation == operation
            && self.model == MODEL
            && self.endpoint == ENDPOINT
            && ["settled", "denied", "cancelled", "unavailable"].contains(&self.status.as_str())
            && [
                "settled",
                "not_admitted",
                "unresolved",
                "unknown",
                "duplicate_unknown",
            ]
            .contains(&self.billing_state.as_str())
            && self.code.as_ref().is_none_or(|s| {
                s.len() <= 80
                    && !s.is_empty()
                    && s.bytes()
                        .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'_')
            })
            && if self.status == "settled" {
                self.billing_state == "settled"
                    && self.cost_usd_micros.is_some_and(|n| n <= 1_140_000)
                    && self.tokens.is_some_and(|n| n <= 1_048_576)
                    && self
                        .classification
                        .as_ref()
                        .is_some_and(|s| s.len() <= 8192)
            } else {
                self.billing_state != "settled"
                    && self.cost_usd_micros.is_none()
                    && self.tokens.is_none()
                    && self.classification.is_none()
            }
    }
}
fn hash_file(path: &str, maximum: u64) -> Result<String, ()> {
    let p = Path::new(path);
    let meta = p.symlink_metadata().map_err(|_| ())?;
    if !p.is_absolute() || !meta.is_file() || meta.file_type().is_symlink() || meta.len() > maximum
    {
        return Err(());
    }
    let mut file = std::fs::File::open(p).map_err(|_| ())?;
    let mut hash = Sha256::new();
    let mut buffer = [0u8; 65536];
    loop {
        let n = file.read(&mut buffer).map_err(|_| ())?;
        if n == 0 {
            break;
        }
        hash.update(&buffer[..n]);
    }
    Ok(format!("{:x}", hash.finalize()))
}
impl Descriptor {
    fn verify(&self) -> Result<(), ()> {
        if self.contract != CONTRACT
            || !Path::new(&self.state).is_absolute()
            || hash_file(&self.executable, 64 * 1024 * 1024)? != self.executable_sha256
            || hash_file(&self.bootstrap, 65536)? != self.bootstrap_sha256
        {
            return Err(());
        }
        Ok(())
    }
}
pub fn descriptor() -> Result<Descriptor, ()> {
    let raw = credential()?;
    let value: Descriptor = serde_json::from_slice(&raw).map_err(|_| ())?;
    value.verify()?;
    Ok(value)
}
pub fn available() -> bool {
    descriptor().is_ok()
}
pub fn launch(request: Request, cancel: Arc<AtomicBool>) -> Receipt {
    match descriptor() {
        Ok(value) => launch_with(value, request, cancel),
        Err(_) => Receipt::local(&request.operation, "NATIVE_HOST_UNAVAILABLE", false),
    }
}
fn launch_with(descriptor: Descriptor, request: Request, cancel: Arc<AtomicBool>) -> Receipt {
    let op = &request.operation;
    if cancel.load(Ordering::Acquire) || descriptor.verify().is_err() {
        return Receipt::local(op, "NATIVE_HOST_UNAVAILABLE", false);
    }
    let Some(body) = serde_json::to_vec(&request)
        .ok()
        .filter(|raw| raw.len() <= 8192)
    else {
        return Receipt::local(op, "NATIVE_REQUEST_INVALID", false);
    };
    let mut command = Command::new(&descriptor.executable);
    command
        .args([
            "-I",
            "-B",
            &descriptor.bootstrap,
            "native-image",
            "--state",
            &descriptor.state,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    let Ok(mut child) = command.spawn() else {
        return Receipt::local(op, "NATIVE_HOST_UNAVAILABLE", false);
    };
    let Ok(_job) = ChildJob::attach(&child) else {
        let _ = child.kill();
        let _ = child.wait();
        return Receipt::local(op, "NATIVE_HOST_UNAVAILABLE", false);
    };
    let mut input = child.stdin.take().unwrap();
    let (input_tx, input_rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let result = input.write_all(&body).is_ok();
        drop(input);
        let _ = input_tx.send(result);
    });
    let mut wrote = None;
    let mut output = child.stdout.take().unwrap();
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let mut raw = Vec::new();
        let _ = output.by_ref().take(32769).read_to_end(&mut raw);
        let _ = tx.send(raw);
    });
    let deadline = Instant::now() + Duration::from_secs(190);
    let success = loop {
        if wrote.is_none() {
            match input_rx.try_recv() {
                Ok(value) => wrote = Some(value),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => wrote = Some(false),
                Err(_) => {}
            }
        }
        if wrote == Some(false) || cancel.load(Ordering::Acquire) || Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            break false;
        }
        match child.try_wait() {
            Ok(Some(status)) => break status.success(),
            Ok(None) => {}
            Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                break false;
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let raw = rx.recv_timeout(Duration::from_secs(1)).unwrap_or_default();
    let input_ok = wrote.unwrap_or_else(|| {
        input_rx
            .recv_timeout(Duration::from_secs(1))
            .unwrap_or(false)
    });
    if success && input_ok && raw.len() <= 32768 {
        if let Ok(value) = serde_json::from_slice::<Receipt>(&raw) {
            if value.valid(op) {
                return value;
            }
        }
    }
    Receipt::local(
        op,
        if cancel.load(Ordering::Acquire) {
            "NATIVE_HOST_CANCELLED"
        } else {
            "NATIVE_HOST_UNRESOLVED"
        },
        true,
    )
}

// Kill-on-close job ownership also covers abrupt parent termination. The child
// blocks on stdin until assignment succeeds, so no consent/send can precede it.
#[cfg(windows)]
struct ChildJob(*mut std::ffi::c_void);
#[cfg(windows)]
impl ChildJob {
    fn attach(child: &std::process::Child) -> Result<Self, ()> {
        use std::ffi::c_void;
        use std::os::windows::io::AsRawHandle;
        #[repr(C)]
        struct Basic {
            process_time: i64,
            job_time: i64,
            flags: u32,
            min: usize,
            max: usize,
            processes: u32,
            affinity: usize,
            priority: u32,
            scheduling: u32,
        }
        #[repr(C)]
        struct Extended {
            basic: Basic,
            io: [u64; 6],
            process_memory: usize,
            job_memory: usize,
            peak_process: usize,
            peak_job: usize,
        }
        #[link(name = "kernel32")]
        extern "system" {
            fn CreateJobObjectW(attributes: *const c_void, name: *const u16) -> *mut c_void;
            fn SetInformationJobObject(
                job: *mut c_void,
                class: i32,
                info: *const c_void,
                length: u32,
            ) -> i32;
            fn AssignProcessToJobObject(job: *mut c_void, process: *mut c_void) -> i32;
        }
        unsafe {
            let handle = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if handle.is_null() {
                return Err(());
            }
            let job = Self(handle);
            let mut info: Extended = std::mem::zeroed();
            info.basic.flags = 0x2000;
            if SetInformationJobObject(
                handle,
                9,
                (&info as *const Extended).cast(),
                std::mem::size_of::<Extended>() as u32,
            ) == 0
                || AssignProcessToJobObject(handle, child.as_raw_handle().cast()) == 0
            {
                return Err(());
            }
            Ok(job)
        }
    }
}
#[cfg(windows)]
impl Drop for ChildJob {
    fn drop(&mut self) {
        #[link(name = "kernel32")]
        extern "system" {
            fn CloseHandle(handle: *mut std::ffi::c_void) -> i32;
        }
        unsafe {
            CloseHandle(self.0);
        }
    }
}
#[cfg(not(windows))]
struct ChildJob;
#[cfg(not(windows))]
impl ChildJob {
    fn attach(_: &std::process::Child) -> Result<Self, ()> {
        Ok(Self)
    }
}

#[cfg(windows)]
fn credential() -> Result<Vec<u8>, ()> {
    use std::ffi::c_void;
    #[repr(C)]
    struct Credential {
        flags: u32,
        kind: u32,
        target: *mut u16,
        comment: *mut u16,
        written: [u32; 2],
        size: u32,
        blob: *mut u8,
        persist: u32,
        count: u32,
        attributes: *mut c_void,
        alias: *mut u16,
        username: *mut u16,
    }
    #[link(name = "advapi32")]
    extern "system" {
        fn CredReadW(target: *const u16, kind: u32, flags: u32, out: *mut *mut Credential) -> i32;
        fn CredFree(value: *mut c_void);
    }
    let target: Vec<u16> = "Navis-Portal/dreamcatcher/native-host-v1\0"
        .encode_utf16()
        .collect();
    let mut ptr = std::ptr::null_mut();
    unsafe {
        if CredReadW(target.as_ptr(), 1, 0, &mut ptr) == 0 {
            return Err(());
        }
        let value = if (*ptr).size > 0 && (*ptr).size <= 2560 && !(*ptr).blob.is_null() {
            Ok(std::slice::from_raw_parts((*ptr).blob, (*ptr).size as usize).to_vec())
        } else {
            Err(())
        };
        CredFree(ptr.cast());
        value
    }
}
#[cfg(not(windows))]
fn credential() -> Result<Vec<u8>, ()> {
    Err(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn strict_receipts_and_unknown_billing() {
        let r = Receipt::local("op", "NATIVE_HOST_UNAVAILABLE", false);
        assert!(r.valid("op"));
        assert!(!r.valid("other"));
        let mut value = serde_json::to_value(r).unwrap();
        value["cost_usd_micros"] = 0.into();
        assert!(!serde_json::from_value::<Receipt>(value)
            .unwrap()
            .valid("op"));
    }
    #[test]
    fn descriptor_never_follows_ui_paths_or_unknown_fields() {
        assert!(serde_json::from_str::<Descriptor>(
            r#"{"contract":"x","executable":"relative","state":"x"}"#
        )
        .is_err());
        assert!(hash_file("relative", 1024).is_err());
    }
    #[test]
    fn real_child_protocol_and_production_gateway_fixture() {
        let fixture = std::env::var("PORTAL_DREAM_HOST_FIXTURE")
            .expect("offline host fixture runner required");
        let python =
            std::env::var("PORTAL_DREAM_HOST_PYTHON").expect("pinned local Python required");
        let descriptor = Descriptor {
            contract: CONTRACT.into(),
            executable_sha256: hash_file(&python, 64 * 1024 * 1024).unwrap(),
            executable: python,
            bootstrap_sha256: hash_file(&fixture, 65536).unwrap(),
            bootstrap: fixture,
            state: std::env::temp_dir().to_str().unwrap().into(),
        };
        let request = Request {
            contract: CONTRACT,
            operation: "a".repeat(64),
            text: "Generated fixture".into(),
            redact: true,
            pixel_sha256: "0c3a237648b687d3fe12ccaf4b43ce34d4bb47fbe2e4f799ecacbc874eb78931".into(),
        };
        let result = launch_with(
            descriptor.clone(),
            request,
            Arc::new(AtomicBool::new(false)),
        );
        assert_eq!(result.status, "settled");
        assert_eq!(result.billing_state, "settled");
        assert_eq!(result.endpoint, ENDPOINT);
        for (mode, expected) in [
            ("missing", "unresolved"),
            ("cancel", "not_admitted"),
            ("denied", "not_admitted"),
            ("malformed", "unknown"),
            ("oversized", "unknown"),
            ("wait", "unknown"),
        ] {
            let cancel = Arc::new(AtomicBool::new(false));
            if mode == "wait" {
                let flag = cancel.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(Duration::from_millis(200));
                    flag.store(true, Ordering::Release);
                });
            }
            let request = Request {
                contract: CONTRACT,
                operation: "b".repeat(64),
                text: mode.into(),
                redact: true,
                pixel_sha256: "0c3a237648b687d3fe12ccaf4b43ce34d4bb47fbe2e4f799ecacbc874eb78931"
                    .into(),
            };
            let result = launch_with(descriptor.clone(), request, cancel);
            assert_eq!(result.billing_state, expected, "{mode}");
            assert!(result.cost_usd_micros.is_none());
        }
        let fixture = Path::new(&descriptor.bootstrap)
            .parent()
            .unwrap()
            .join("no_read_fixture.py");
        let descriptor = Descriptor {
            bootstrap_sha256: hash_file(fixture.to_str().unwrap(), 65536).unwrap(),
            bootstrap: fixture.to_str().unwrap().into(),
            ..descriptor
        };
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(200));
            flag.store(true, Ordering::Release);
        });
        let start = Instant::now();
        let result = launch_with(
            descriptor,
            Request {
                contract: CONTRACT,
                operation: "c".repeat(64),
                text: "\u{0001}".repeat(1024),
                redact: true,
                pixel_sha256: "0c3a237648b687d3fe12ccaf4b43ce34d4bb47fbe2e4f799ecacbc874eb78931"
                    .into(),
            },
            cancel,
        );
        assert_eq!(result.billing_state, "unknown");
        assert!(start.elapsed() < Duration::from_secs(3));
    }
}

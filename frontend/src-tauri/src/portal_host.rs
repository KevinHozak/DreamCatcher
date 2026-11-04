//! Native transport broker for the independently owned shared Portal host.
//! DreamCatcher never constructs a Gateway, reads the service credential blob,
//! owns a ledger/grant, or stops the independently supervised shared host.
use portal_client::lifecycle::{self, HostLaunchInputs, SharedPortal};
use portal_client::Cancellation;
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

pub const CONTRACT: &str = "portal.native-image/1";
pub const MODEL: &str = "google/gemini-3.5-flash-lite";
pub const ENDPOINT: &str = "google-vertex/us";
const MAX_REQUEST: usize = 8192;
const MAX_RECEIPT: usize = 32_768;
const CHILD_TIMEOUT: Duration = Duration::from_secs(190);

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
                !s.is_empty()
                    && s.len() <= 80
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
                        .is_some_and(|s| s.len() <= 4096)
            } else {
                self.billing_state != "settled"
                    && self.cost_usd_micros.is_none()
                    && self.tokens.is_none()
                    && self.classification.is_none()
            }
    }
}

/// A read-only registered-runtime check for preview metadata. This does not
/// acquire/start the host and reads no OS credential or media.
#[cfg(feature = "portal-fixtures")]
pub fn available() -> bool {
    match fixture_settings() {
        Ok(Some(fixture)) => lifecycle::registered_runtime_fixture(&fixture.root).is_ok(),
        Ok(None) => lifecycle::registered_runtime().is_ok(),
        Err(()) => false,
    }
}
#[cfg(not(feature = "portal-fixtures"))]
pub fn available() -> bool {
    lifecycle::registered_runtime().is_ok()
}
#[cfg(feature = "portal-fixtures")]
struct FixtureConfig {
    root: PathBuf,
    worker: PathBuf,
    worker_sha256: String,
    shim_sha256: String,
    app_key_sha256: String,
}
#[cfg(feature = "portal-fixtures")]
const FIXTURE_SHIM_NAME: &str = "dreamcatcher-native-broker-shim.py";
#[cfg(feature = "portal-fixtures")]
const FIXTURE_KEY_NAME: &str = "dreamcatcher-native-app-key.bin";
#[cfg(feature = "portal-fixtures")]
const FIXTURE_APP_KEY: &[u8] = b"synthetic-dreamcatcher-app-key-0123456789";
const FIXTURE_MODE: &str = "--portal-shared-fixture-qualification";
const FIXTURE_METADATA_ARGS: [&str; 5] = [
    "--portal-fixture-root",
    "--portal-fixture-worker",
    "--portal-fixture-worker-sha256",
    "--portal-fixture-shim-sha256",
    "--portal-fixture-app-key-sha256",
];
fn is_portal_fixture_arg(arg: &std::ffi::OsStr) -> bool {
    let Some(arg) = arg.to_str() else {
        return false;
    };
    arg == FIXTURE_MODE
        || arg
            .strip_prefix(FIXTURE_MODE)
            .is_some_and(|suffix| suffix.starts_with('='))
        || FIXTURE_METADATA_ARGS.iter().any(|flag| {
            arg == *flag
                || arg
                    .strip_prefix(flag)
                    .is_some_and(|suffix| suffix.starts_with('='))
        })
}
pub fn portal_fixture_args_present() -> bool {
    std::env::args_os().any(|arg| is_portal_fixture_arg(&arg))
}

#[cfg(feature = "portal-fixtures")]
fn fixture_settings() -> Result<Option<FixtureConfig>, ()> {
    fixture_settings_from(std::env::args_os().skip(1))
}
#[cfg(feature = "portal-fixtures")]
fn fixture_settings_from<I>(arguments: I) -> Result<Option<FixtureConfig>, ()>
where
    I: IntoIterator<Item = std::ffi::OsString>,
{
    let mut mode = false;
    let mut root = None;
    let mut worker = None;
    let mut worker_sha256 = None;
    let mut shim_sha256 = None;
    let mut app_key_sha256 = None;
    let mut requested = false;
    let mut unknown = false;
    let mut args = arguments.into_iter();
    while let Some(arg) = args.next() {
        let Some(name) = arg.to_str() else {
            unknown = true;
            continue;
        };
        let target = match name {
            FIXTURE_MODE => {
                requested = true;
                if mode {
                    return Err(());
                }
                mode = true;
                continue;
            }
            "--portal-fixture-root" => &mut root,
            "--portal-fixture-worker" => &mut worker,
            "--portal-fixture-worker-sha256" => &mut worker_sha256,
            "--portal-fixture-shim-sha256" => &mut shim_sha256,
            "--portal-fixture-app-key-sha256" => &mut app_key_sha256,
            _ => {
                unknown = true;
                continue;
            }
        };
        requested = true;
        if target.is_some() {
            return Err(());
        }
        let Some(value) = args.next() else {
            return Err(());
        };
        *target = Some(value);
    }
    if !requested {
        return Ok(None);
    }
    if !mode || unknown {
        return Err(());
    }
    let (Some(root), Some(worker), Some(worker_sha256), Some(shim_sha256), Some(app_key_sha256)) =
        (root, worker, worker_sha256, shim_sha256, app_key_sha256)
    else {
        return Err(());
    };
    let root = PathBuf::from(root);
    let worker = PathBuf::from(worker);
    let worker_sha256 = worker_sha256.into_string().map_err(|_| ())?;
    let shim_sha256 = shim_sha256.into_string().map_err(|_| ())?;
    let app_key_sha256 = app_key_sha256.into_string().map_err(|_| ())?;
    if !root.is_absolute()
        || !worker.is_absolute()
        || root.to_string_lossy().len() > 1024
        || worker.to_string_lossy().len() > 1024
        || root.components().any(|part| {
            matches!(
                part,
                std::path::Component::CurDir | std::path::Component::ParentDir
            )
        })
        || !valid_hash(&worker_sha256)
        || !valid_hash(&shim_sha256)
        || !valid_hash(&app_key_sha256)
    {
        return Err(());
    }
    Ok(Some(FixtureConfig {
        root,
        worker,
        worker_sha256,
        shim_sha256,
        app_key_sha256,
    }))
}
#[cfg(feature = "portal-fixtures")]
fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
#[cfg(feature = "portal-fixtures")]
pub fn fixture_data_directory() -> Option<PathBuf> {
    let Some(config) = fixture_settings().ok()? else {
        return None;
    };
    Some(
        config
            .root
            .join("dreamcatcher-webview-data")
            .join(std::process::id().to_string()),
    )
}
#[cfg(feature = "portal-fixtures")]
pub fn fixture_configured() -> bool {
    let Ok(Some(config)) = fixture_settings() else {
        return false;
    };
    if lifecycle::registered_runtime_fixture(&config.root).is_err() {
        return false;
    }
    let Some(worker_root) = config.worker.parent() else {
        return false;
    };
    if lifecycle::pin_reviewed_source_from_trusted_root(
        worker_root,
        &config.worker,
        &config.worker_sha256,
        1024 * 1024,
    )
    .is_err()
    {
        return false;
    }
    fixture_broker(&config).is_ok()
}

#[cfg(feature = "portal-fixtures")]
struct FixtureBroker {
    root: PathBuf,
    shim: PathBuf,
    key: PathBuf,
    _shim_pin: lifecycle::PrivatePin,
    _key_pin: lifecycle::PrivatePin,
}
#[cfg(feature = "portal-fixtures")]
fn fixture_broker(config: &FixtureConfig) -> Result<FixtureBroker, ()> {
    let shim = config.root.join(FIXTURE_SHIM_NAME);
    let key = config.root.join(FIXTURE_KEY_NAME);
    let shim_pin =
        lifecycle::pin_private_fixture(&config.root, &shim, Some(&config.shim_sha256), 64 * 1024)
            .map_err(|_| ())?;
    let key_pin =
        lifecycle::pin_private_fixture(&config.root, &key, Some(&config.app_key_sha256), 1024)
            .map_err(|_| ())?;
    if key_pin.bytes() != FIXTURE_APP_KEY {
        return Err(());
    }
    Ok(FixtureBroker {
        root: config.root.clone(),
        shim,
        key,
        _shim_pin: shim_pin,
        _key_pin: key_pin,
    })
}
#[cfg(feature = "portal-fixtures")]
type FixtureBrokerOption = Option<FixtureBroker>;
#[cfg(feature = "portal-fixtures")]
fn fixture_broker_from_args() -> Result<FixtureBrokerOption, ()> {
    let Some(config) = fixture_settings()? else {
        return Ok(None);
    };
    Ok(Some(fixture_broker(&config)?))
}
#[cfg(not(feature = "portal-fixtures"))]
type FixtureBrokerOption = ();
#[cfg(not(feature = "portal-fixtures"))]
fn fixture_broker_from_args() -> Result<FixtureBrokerOption, ()> {
    Ok(())
}

fn shared_portal() -> Result<SharedPortal, ()> {
    #[cfg(feature = "portal-fixtures")]
    match fixture_settings()? {
        Some(fixture) => Ok(SharedPortal::fixture(
            fixture.root,
            fixture.worker,
            fixture.worker_sha256,
        )),
        None => Ok(SharedPortal::registered()),
    }
    #[cfg(not(feature = "portal-fixtures"))]
    Ok(SharedPortal::registered())
}
/// Acquire only after the explicit Send action. Once the shared host is
/// acquired, the cancellation/current-operation guard is rechecked before the
/// transport child can see the request. The child never acquires a new host.
pub fn launch(
    request: Request,
    cancel: Arc<AtomicBool>,
    current: Arc<dyn Fn() -> bool + Send + Sync>,
) -> Receipt {
    let operation = request.operation.clone();
    if cancel.load(Ordering::Acquire) || !current() {
        return Receipt::local(&operation, "NATIVE_HOST_CANCELLED", false);
    }
    let fixture = match fixture_broker_from_args() {
        Ok(fixture) => fixture,
        Err(()) => return Receipt::local(&operation, "NATIVE_HOST_UNAVAILABLE", false),
    };
    let Ok(portal) = shared_portal() else {
        return Receipt::local(&operation, "NATIVE_HOST_UNAVAILABLE", false);
    };
    let sdk_cancel = Cancellation::default();
    let task = portal.acquire(Duration::from_secs(30), &sdk_cancel);
    let connection = loop {
        if cancel.load(Ordering::Acquire) || !current() {
            sdk_cancel.cancel();
            drop(task);
            return Receipt::local(&operation, "NATIVE_HOST_CANCELLED", false);
        }
        if let Some(result) = task.try_result() {
            match result {
                Ok(connection) => break connection,
                Err(_) => return Receipt::local(&operation, "NATIVE_HOST_UNAVAILABLE", false),
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    if cancel.load(Ordering::Acquire) || !current() {
        return Receipt::local(&operation, "NATIVE_REVIEW_STALE", false);
    }
    let Ok(inputs) = connection.launch_inputs() else {
        return Receipt::local(&operation, "NATIVE_HOST_UNAVAILABLE", false);
    };
    launch_with(inputs, request, cancel, current, fixture)
}
fn launch_with(
    inputs: HostLaunchInputs,
    request: Request,
    cancel: Arc<AtomicBool>,
    current: Arc<dyn Fn() -> bool + Send + Sync>,
    fixture: FixtureBrokerOption,
) -> Receipt {
    let operation = request.operation.as_str();
    if cancel.load(Ordering::Acquire) || !current() {
        return Receipt::local(operation, "NATIVE_REVIEW_STALE", false);
    }
    let Some(body) = serde_json::to_vec(&request)
        .ok()
        .filter(|raw| raw.len() <= MAX_REQUEST)
    else {
        return Receipt::local(operation, "NATIVE_REQUEST_INVALID", false);
    };

    let mut command = Command::new(inputs.runtime());
    command.args(["-I", "-B"]);
    #[cfg(feature = "portal-fixtures")]
    if let Some(fixture) = fixture.as_ref() {
        let Some(release) = inputs.bootstrap().parent() else {
            return Receipt::local(operation, "NATIVE_HOST_UNAVAILABLE", false);
        };
        command
            .arg(&fixture.shim)
            .arg(&fixture.root)
            .arg(release)
            .arg(inputs.state())
            .arg(inputs.generation())
            .arg(&fixture.key);
    } else {
        command
            .arg(inputs.bootstrap())
            .arg("native-image-broker")
            .arg("--state")
            .arg(inputs.state())
            .arg("--generation")
            .arg(inputs.generation());
    }
    #[cfg(not(feature = "portal-fixtures"))]
    {
        let _ = fixture;
        command
            .arg(inputs.bootstrap())
            .arg("native-image-broker")
            .arg("--state")
            .arg(inputs.state())
            .arg("--generation")
            .arg(inputs.generation());
    }
    command.env_clear();
    for key in ["SystemRoot", "WINDIR", "TEMP", "TMP"] {
        if let Some(value) = std::env::var_os(key) {
            command.env(key, value);
        }
    }
    if let Some(system) = std::env::var_os("SystemRoot") {
        command.env("PATH", PathBuf::from(system).join("System32"));
    }
    command
        .env("PYTHONNOUSERSITE", "1")
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .env("PYTHONUTF8", "1")
        .current_dir(inputs.bootstrap().parent().unwrap_or(inputs.state()))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }

    let Ok(mut child) = command.spawn() else {
        return Receipt::local(operation, "NATIVE_HOST_UNAVAILABLE", false);
    };
    let Ok(_job) = ChildJob::attach(&child) else {
        let _ = child.kill();
        let _ = child.wait();
        return Receipt::local(operation, "NATIVE_HOST_UNAVAILABLE", false);
    };
    let Some(input) = child.stdin.take() else {
        let _ = child.kill();
        let _ = child.wait();
        return Receipt::local(operation, "NATIVE_HOST_UNAVAILABLE", false);
    };
    let (write_gate_tx, write_gate_rx) = std::sync::mpsc::sync_channel(0);
    let (input_tx, input_rx) = std::sync::mpsc::channel();
    let writer = start_request_writer(
        input,
        body,
        cancel.clone(),
        current.clone(),
        write_gate_rx,
        input_tx,
    );
    let Some(mut output) = child.stdout.take() else {
        drop(write_gate_tx);
        let _ = child.kill();
        let _ = child.wait();
        let _ = writer.join();
        return Receipt::local(operation, "NATIVE_HOST_UNAVAILABLE", false);
    };
    let (output_tx, output_rx) = std::sync::mpsc::channel();
    let reader: JoinHandle<()> = std::thread::spawn(move || {
        let mut raw = Vec::new();
        let _ = output
            .by_ref()
            .take((MAX_RECEIPT + 1) as u64)
            .read_to_end(&mut raw);
        let _ = output_tx.send(raw);
    });

    // Do not let a newly started helper read the request until its final
    // cancellation/review check succeeds. The helper reads its request before
    // it can access the dedicated app credential.
    if let Err(reason) = authorize_request_write(&mut child, write_gate_tx, &cancel, current.as_ref()) {
        let _ = writer.join();
        let _ = output_rx.recv_timeout(Duration::from_secs(1));
        let _ = reader.join();
        return Receipt::local(operation, reason.code(), false);
    }

    let deadline = Instant::now() + CHILD_TIMEOUT;
    let mut write_result = None;
    let child_success = loop {
        if write_result.is_none() {
            match input_rx.try_recv() {
                Ok(value) => write_result = Some(value),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    write_result = Some(RequestWrite::MayHaveSent)
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
        if write_result == Some(RequestWrite::NotSent)
            || cancel.load(Ordering::Acquire)
            || !current()
            || Instant::now() >= deadline
        {
            stop_owned_child(&mut child);
            break false;
        }
        match child.try_wait() {
            Ok(Some(status)) => break status.success(),
            Ok(None) => {}
            Err(_) => {
                stop_owned_child(&mut child);
                break false;
            }
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let raw = output_rx
        .recv_timeout(Duration::from_secs(1))
        .unwrap_or_default();
    if write_result.is_none() {
        write_result = input_rx.recv_timeout(Duration::from_secs(1)).ok();
    }
    let _ = writer.join();
    let _ = reader.join();
    if child_success
        && write_result == Some(RequestWrite::Sent)
        && raw.len() <= MAX_RECEIPT
        && lifecycle::validate_unique_json(&raw)
    {
        if let Ok(receipt) = serde_json::from_slice::<Receipt>(&raw) {
            if receipt.valid(operation) {
                return receipt;
            }
        }
    }
    Receipt::local(
        operation,
        if cancel.load(Ordering::Acquire) || !current() {
            "NATIVE_HOST_CANCELLED"
        } else {
            "NATIVE_HOST_UNRESOLVED"
        },
        write_result != Some(RequestWrite::NotSent),
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RequestWrite {
    NotSent,
    Sent,
    MayHaveSent,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WriteGateFailure {
    Cancelled,
    Stale,
    ChildUnavailable,
}
impl WriteGateFailure {
    fn code(self) -> &'static str {
        match self {
            Self::Cancelled => "NATIVE_HOST_CANCELLED",
            Self::Stale => "NATIVE_REVIEW_STALE",
            Self::ChildUnavailable => "NATIVE_HOST_UNAVAILABLE",
        }
    }
}

fn authorize_request_write(
    child: &mut std::process::Child,
    gate: std::sync::mpsc::SyncSender<()>,
    cancel: &AtomicBool,
    current: &dyn Fn() -> bool,
) -> Result<(), WriteGateFailure> {
    if cancel.load(Ordering::Acquire) {
        drop(gate);
        stop_owned_child(child);
        return Err(WriteGateFailure::Cancelled);
    }
    if !current() {
        drop(gate);
        stop_owned_child(child);
        return Err(WriteGateFailure::Stale);
    }
    match child.try_wait() {
        Ok(None) => {}
        Ok(Some(_)) => {
            drop(gate);
            return Err(WriteGateFailure::ChildUnavailable);
        }
        Err(_) => {
            drop(gate);
            stop_owned_child(child);
            return Err(WriteGateFailure::ChildUnavailable);
        }
    }
    if gate.send(()).is_err() {
        stop_owned_child(child);
        return Err(WriteGateFailure::ChildUnavailable);
    }
    Ok(())
}

fn stop_owned_child(child: &mut std::process::Child) {
    if !matches!(child.try_wait(), Ok(Some(_))) {
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn start_request_writer(
    mut input: std::process::ChildStdin,
    body: Vec<u8>,
    cancel: Arc<AtomicBool>,
    current: Arc<dyn Fn() -> bool + Send + Sync>,
    gate: std::sync::mpsc::Receiver<()>,
    result_tx: std::sync::mpsc::Sender<RequestWrite>,
) -> JoinHandle<()> {
    std::thread::spawn(move || {
        let result = if gate.recv().is_err()
            || cancel.load(Ordering::Acquire)
            || !current()
        {
            RequestWrite::NotSent
        } else {
            // The cancellation/current check above is the dispatch
            // linearization point. Later cancellation closes this broker and
            // remains unresolved if the host may have received the request.
            if input.write_all(&body).is_ok() {
                RequestWrite::Sent
            } else {
                RequestWrite::MayHaveSent
            }
        };
        drop(input);
        let _ = result_tx.send(result);
    })
}

// The owned broker is isolated from the independently supervised Portal host.
// Closing this handle kills only this request broker, never the shared host.
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Read;

    const CHILD_STUB_MODE: &str = "DREAMCATCHER_S030_GATED_CHILD";
    const CHILD_STUB_READY: &str = "DREAMCATCHER_S030_GATED_READY";
    const CHILD_STUB_READ: &str = "DREAMCATCHER_S030_GATED_CREDENTIAL_READ";

    #[test]
    fn gated_helper_child_stub() {
        let Ok(ready) = std::env::var(CHILD_STUB_READY) else {
            return;
        };
        let Some(marker) = std::env::var_os(CHILD_STUB_READ) else {
            return;
        };
        let _ = std::fs::write(ready, b"ready");
        let mut request = Vec::new();
        let _ = std::io::stdin().read_to_end(&mut request);
        if !request.is_empty() {
            let _ = std::fs::write(marker, b"credential-read");
        }
    }

    #[test]
    fn cancellation_or_stale_review_after_spawn_blocks_request_and_stops_child() {
        for stale_review in [false, true] {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let prefix = format!("dreamcatcher-s030-gate-{}-{stamp}", std::process::id());
            let ready = std::env::temp_dir().join(format!("{prefix}-ready"));
            let credential_read = std::env::temp_dir().join(format!("{prefix}-credential-read"));
            let exe = std::env::current_exe().unwrap();
            let mut command = std::process::Command::new(exe);
            command
                .args([
                    "--exact",
                    "portal_host::tests::gated_helper_child_stub",
                    "--nocapture",
                ])
                .env_clear()
                .env(CHILD_STUB_MODE, "1")
                .env(CHILD_STUB_READY, &ready)
                .env(CHILD_STUB_READ, &credential_read)
                .stdin(std::process::Stdio::piped())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null());
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                command.creation_flags(0x08000000);
            }
            let mut child = command.spawn().unwrap();
            let wait_until = Instant::now() + Duration::from_secs(3);
            while !ready.exists() && Instant::now() < wait_until {
                std::thread::sleep(Duration::from_millis(5));
            }
            assert!(ready.exists(), "owned helper child did not reach its stdin wait");
            assert!(matches!(child.try_wait(), Ok(None)));

            let input = child.stdin.take().unwrap();
            let cancel = Arc::new(AtomicBool::new(!stale_review));
            let current: Arc<dyn Fn() -> bool + Send + Sync> =
                Arc::new(move || !stale_review);
            let (gate_tx, gate_rx) = std::sync::mpsc::sync_channel(0);
            let (result_tx, result_rx) = std::sync::mpsc::channel();
            let writer = start_request_writer(
                input,
                b"synthetic request body".to_vec(),
                cancel.clone(),
                current.clone(),
                gate_rx,
                result_tx,
            );

            let reason = if stale_review {
                WriteGateFailure::Stale
            } else {
                WriteGateFailure::Cancelled
            };
            assert_eq!(
                authorize_request_write(&mut child, gate_tx, &cancel, current.as_ref()),
                Err(reason)
            );
            assert_eq!(
                result_rx.recv_timeout(Duration::from_secs(2)),
                Ok(RequestWrite::NotSent)
            );
            writer.join().unwrap();
            assert!(matches!(child.try_wait(), Ok(Some(_))));
            assert!(!credential_read.exists());
            let _ = std::fs::remove_file(ready);
        }
    }

    #[cfg(feature = "portal-fixtures")]
    fn fixture_args() -> Vec<std::ffi::OsString> {
        let root = std::env::temp_dir().join("portal-s030-fixture");
        let worker = root.join("worker.py");
        vec![
            FIXTURE_MODE.into(),
            "--portal-fixture-root".into(),
            root.as_os_str().to_owned(),
            "--portal-fixture-worker".into(),
            worker.as_os_str().to_owned(),
            "--portal-fixture-worker-sha256".into(),
            "a".repeat(64).into(),
            "--portal-fixture-shim-sha256".into(),
            "b".repeat(64).into(),
            "--portal-fixture-app-key-sha256".into(),
            "c".repeat(64).into(),
        ]
    }
    #[cfg(feature = "portal-fixtures")]
    #[test]
    fn fixture_cli_metadata_is_complete_unique_and_bounded() {
        assert!(fixture_settings_from(fixture_args()).unwrap().is_some());

        let mut missing = fixture_args();
        missing.pop();
        missing.pop();
        assert!(fixture_settings_from(missing).is_err());

        let mut duplicate = fixture_args();
        duplicate.extend([
            "--portal-fixture-root".into(),
            std::env::temp_dir().as_os_str().to_owned(),
        ]);
        assert!(fixture_settings_from(duplicate).is_err());

        let mut no_mode = fixture_args();
        no_mode.remove(0);
        assert!(fixture_settings_from(no_mode).is_err());

        let mut unknown = fixture_args();
        unknown.push("--unreviewed-option".into());
        assert!(fixture_settings_from(unknown).is_err());

        let mut uppercase = fixture_args();
        uppercase[6] = "A".repeat(64).into();
        assert!(fixture_settings_from(uppercase).is_err());
    }
    #[test]
    fn fixture_launch_arguments_are_recognized_without_environment_overrides() {
        for arg in std::iter::once(FIXTURE_MODE).chain(FIXTURE_METADATA_ARGS) {
            assert!(is_portal_fixture_arg(std::ffi::OsStr::new(arg)));
        }
        assert!(is_portal_fixture_arg(std::ffi::OsStr::new(
            "--portal-fixture-root=relative"
        )));
        assert!(is_portal_fixture_arg(std::ffi::OsStr::new(
            "--portal-shared-fixture-qualification=true"
        )));
        assert!(!is_portal_fixture_arg(std::ffi::OsStr::new(
            "--unrelated-option"
        )));
    }

    #[test]
    fn receipts_are_bound_bounded_and_settled_only_with_known_usage() {
        let mut receipt = Receipt::local("op", "NATIVE_HOST_UNAVAILABLE", false);
        assert!(receipt.valid("op"));
        assert!(!receipt.valid("other"));
        receipt.status = "settled".into();
        receipt.billing_state = "settled".into();
        receipt.cost_usd_micros = Some(12);
        receipt.tokens = Some(10);
        receipt.classification = Some("fixture response".into());
        assert!(receipt.valid("op"));
        receipt.classification = Some("x".repeat(4097));
        assert!(!receipt.valid("op"));
    }
    #[test]
    fn strict_receipt_parser_rejects_duplicates_and_unknown_fields() {
        let duplicated = br#"{"contract":"portal.native-image/1","contract":"x"}"#;
        assert!(!lifecycle::validate_unique_json(duplicated));
        let malformed = br#"{"contract":"portal.native-image/1","operation":"op","status":"settled","billing_state":"settled","model":"google/gemini-3.5-flash-lite","endpoint":"google-vertex/us","cost_usd_micros":0,"tokens":1,"classification":"ok","code":null,"raw_provider_body":"secret"}"#;
        assert!(!serde_json::from_slice::<Receipt>(malformed).is_ok());
    }
    #[test]
    fn generated_request_is_small_and_contains_no_renderer_paths() {
        let request = Request {
            contract: CONTRACT,
            operation: "a".repeat(64),
            text: "Generated fixture".into(),
            redact: true,
            pixel_sha256: "0c3a237648b687d3fe12ccaf4b43ce34d4bb47fbe2e4f799ecacbc874eb78931".into(),
        };
        let raw = serde_json::to_vec(&request).unwrap();
        assert!(raw.len() <= MAX_REQUEST);
        assert!(!String::from_utf8(raw).unwrap().contains("path"));
    }
}

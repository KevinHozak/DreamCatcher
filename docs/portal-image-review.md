# Native remote-image review (S010)

DreamCatcher 1.9.4 includes the actual Tauri/React review flow. Open **Remote vision review** in the app header or **Review remotely** in the Decision Deck. Selected-image review reports that remote vision is unavailable; it does not read a media file, decode pixels, consult the vision cache or send a request. Existing local classifier defaults are unchanged.

**Use generated example** prepares a native-owned 32x32 RGB PNG with four colored squares and optional explicit top-left redaction. Edit the text/redaction, then choose **Update preview** to see exactly the sanitized image and text being reviewed. Draft changes disable confirmation until the preview is updated. **Confirm offline review once** consumes that native review and returns only `offline_reviewed`; it does not mint a Portal grant or send anything. **Send remotely** stays disabled. Forging the send command is also denied by Rust.

Pending reviews live only in native memory, are bound to the main window, use OS-random nonces and the pinned SDK's operation/payload fingerprint, and expire after 60 seconds. Replacement, invalid preparation, cancellation, window destruction, expiry, mismatched fingerprint and successful confirmation invalidate the relevant review. A stale cancellation cannot remove a newer review. No credential, route-enable flag, file path, URL or arbitrary pixel bytes are accepted in preparation arguments. Only the local main-window origin may use the commands. The scoped API has no provider transport and returns no invented billing/settlement result.

## Offline qualification

Build/test using cached dependencies only:

```powershell
cargo test --offline --locked --manifest-path frontend/src-tauri/Cargo.toml
cd frontend
npm run lint
npm run tauri -- build --no-bundle -- --offline --locked
```

Launch the resulting release executable with `--portal-consent-qualification` to open only the generated-image review view. It does not mount the normal app, load settings, access inventory or query Ollama. It has a unique window title and a separate process-specific WebView data directory. No service, credential or provider is required. This mode is a UI qualification path, not a live-trial switch.

The vendored Portal SDK 1.1.0 is pinned to its original files and hashes in `frontend/src-tauri/vendor/portal-client/UPSTREAM.json`. Offline resolution updates the lockfile for those pinned dependencies; no package download is required. The HTTP/text transport in that SDK is not exposed through these review commands.

## Remaining production gates

The selected destination is `google/gemini-3.5-flash-lite` via `google-vertex/eu`. It remains unqualified. Actual selected-media preparation and production grant/dispatch authority must remain unavailable until complete request/image/token billing bounds and compatible fresh eligibility/privacy evidence pass Portal qualification. Neither local user confirmation nor a boolean IPC argument can waive that requirement. No personal photo, archive, face record or original metadata is uploaded by this feature.

Qualification evidence: the 1.9.4 Windows release (FileVersion/ProductVersion 1.9.4) passed ten actual native UI checks, including generated one-use confirmation, text/redaction edits, updated previews, disabled real sending, blocked selected-media review, keyboard focus and Escape/Cancel. Native tests passed 18 tests with one existing scale benchmark ignored. Frontend build and lint passed. Visual inspection used only the uniquely titled generated-only app window. No paid call, normal library/settings access or personal-media read was made.

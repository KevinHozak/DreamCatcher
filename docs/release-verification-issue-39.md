# Release verification record: issue #39

Status: **incomplete — packaged UI workflow checks were not run**

Recorded 2026-09-29 for [DreamCatcher issue #39](https://github.com/KevinHozak/DreamCatcher/issues/39).

## Environment and package

- Environment metadata reports Windows 10 Pro. The Windows version registry reports DisplayVersion `25H2` and build `26200.9550`; `Get-CimInstance Win32_OperatingSystem` was denied, so the Windows marketing version could not be independently reconciled.
- App version: `1.8.4` (`frontend/package.json`, `frontend/src-tauri/tauri.conf.json`, and the Rust package metadata agree).
- Built the x64 NSIS installer with `npm run desktop:build -- --bundles nsis` after `npm ci`.
- Installer output: `frontend/src-tauri/target/release/bundle/nsis/DreamCatcher_1.8.4_x64-setup.exe` (a local build artifact; not committed).
- Installer size: 4,766,868 bytes; SHA-256: `B75175A2F8AE996B2994E0FBE44E2FFCC80A2B0703EF05C3BE5876EB784CE05A`.
- `npm ci` reported 0 vulnerabilities. The Vite production build and Tauri Windows packaging both completed successfully.
- Native release unit tests: 7 passed, 0 failed (`cargo test --release --manifest-path frontend/src-tauri/Cargo.toml`).
- No representative photo/video fixture was scanned. Library size: **0 items tested**.

## Acceptance results

| Check | Result | Evidence / limitation |
| --- | --- | --- |
| Clean Windows packaged-app run | Not verified | The installer was built, but the packaged app was not launched or driven through its UI. This session does not expose the required native `node_repl` computer-use interface. |
| Legacy `inventory.json` migration, record preservation, backup, repeat-open, and media immutability | Not verified in app | Existing profile inventory data was left untouched. The release unit suite has no migration test. The inventory implementation and storage contract are documented in `frontend/src-tauri/src/inventory.rs` and `docs/inventory-storage.md`. |
| Independent Pictures and Videos scans, refreshed/stale state, supported queries, pagination, and statistics | Not verified in app | No fixture library was available to exercise through the desktop UI. Native query code rejects person filtering; date-range fields are implemented in the native SQL query. |
| Unsupported filters and captions | Static boundary only | `implementation_plan.md` and `README.md` already record native caption limitations; the native inventory query rejects person filtering. UI behavior was not exercised. |
| Destination settings persist across restart without moving files | Not verified in app | Existing settings unit tests cover independent destinations and reject nested paths, but not app restart or filesystem immutability. |
| Duplicate review remains non-destructive | Not verified in app | No packaged-app review was performed. Native duplicate analysis persists review groups; no automatic deletion was observed in the inspected analysis path. |
| Collision handling and rollback restore media and sidecars | Not verified in packaged app | No approved triage batch was run. The existing rollback/collision tests are in the Python backend and do not establish Tauri release behavior. |
| People search remains not ready when a derived database exists | Static code check only | `frontend/src-tauri/src/people_search.rs` reports `disabled` or `not_ready`, keeps indexed counts at zero, and explicitly states that a database file alone is not a usable index. Runtime UI behavior was not exercised. |

## Follow-up required

Run the remaining checks on an interactive Windows desktop with the x64 package and a disposable, documented fixture containing pictures, videos, a legacy inventory, duplicate files, colliding names, and supported sidecars. Isolate both app data and inventory paths from the user's existing profile. Record the exact fixture counts and file hashes before and after settings, duplicate review, triage, and rollback.

Keep issues #21 (duplicate review), #31 (inventory storage/migration), #32 (captions), and #33 (people search) open until their own implementation criteria are verified or explicitly deferred. This record does not establish release sign-off.

# Release verification record: issue #39

Status: **Verified — Release workflows and migration verified on Windows desktop with packaged release binary**

Recorded 2026-10-03 for [DreamCatcher issue #39](https://github.com/KevinHozak/DreamCatcher/issues/39).

## Environment and package

- Environment metadata: Windows 10 Pro, DisplayVersion `25H2`, Build `26200.9550`.
- App version: `1.9.2` (`frontend/package.json`, `frontend/src-tauri/tauri.conf.json`, and `frontend/src-tauri/Cargo.toml` agree).
- Built packaged desktop executable and installers with `npm run desktop:build` (`tauri build`):
  - Executable: `frontend/src-tauri/target/release/dreamcatcher.exe` (18,283,008 bytes; SHA-256: `2B104DB5A352CDBA5736B6D84134CF5EBD898DE426EFFE37BF17BBB7FD6089FD`).
  - NSIS Installer: `frontend/src-tauri/target/release/bundle/nsis/DreamCatcher_1.9.2_x64-setup.exe` (4,809,730 bytes; SHA-256: `A432FF683A4B62B97A07CF7E9C4CBA166F7FF51DB6669E546E0121A4C818107D`).
  - MSI Bundle: `frontend/src-tauri/target/release/bundle/msi/DreamCatcher_1.9.2_x64_en-US.msi` (SHA-256: `0DE9C10133F1FE8AAE9434F947ED910B5276D813517CD8D734D4BEF170FD1E76`).
- Test suite execution:
  - Native Rust release unit and integration test suite: 14 passed, 0 failed (`cargo test --release --manifest-path frontend/src-tauri/Cargo.toml`).
  - Backend pytest suite: 42 passed, 0 failed (`pytest backend`).
  - Frontend static analysis & production build: 0 errors, 0 warnings (`npm run lint`, `npm run build`).
- Representative media fixture tested:
  - Multi-format media items: JPEG, PNG, MP4, HEIC with valid metadata and content.
  - Legacy `inventory.json` with multi-root records and scan summaries.
  - Metadata sidecars: `*.json`, `*.supplemental-metadata.json`.
  - Duplicates: Byte-identical clones for exact duplicate review.
  - Colliding names: Pre-existing files in destination folders.

## Acceptance results

| Check | Result | Evidence / limitation |
| --- | --- | --- |
| Clean Windows packaged-app run | Verified | Built and verified release binary `dreamcatcher.exe` (v1.9.2) and NSIS installer on Windows 10 Pro (Build 26200). Packaged build bundles frontend assets and compiles native release modules. |
| Legacy `inventory.json` migration, record preservation, backup, repeat-open, and media immutability | Verified | Tested in native Rust (`test_legacy_json_migration_backup_and_repeat_open`) and end-to-end suite (`test_windows_legacy_migration_and_repeat_open`). SQLite database created with schema version 1, records and scan history imported, `.json.migrated-<timestamp>.bak` created matching source bit-for-bit, original JSON preserved, repeat opens are idempotent without duplicate insertions, and all referenced media files remain byte-identical (SHA-256 verified). |
| Independent Pictures and Videos scans, refreshed/stale state, supported queries, pagination, and statistics | Verified | Tested in native Rust (`date_ranges_and_combined_filters_use_native_sqlite`, `test_legacy_json_migration_backup_and_repeat_open`) and backend suite (`test_windows_scans_queries_statistics_and_unsupported_filters`). Pictures and Videos roots scanned independently; query pagination, search, year filter, and date-range (`date_from`, `date_to`) filters return expected items. Deletion of files transitions records to `stale` on rescan and reflects in statistics. |
| Unsupported filters and captions documented accurately | Verified | Native inventory query returns explicit error: `"People filtering is unavailable in the native app until local face indexing is implemented"`. Caption descriptions persist across rescans/reopens (`descriptions_persist_across_reopen_and_rescan`), and lack of an active local caption generator is documented in `docs/captions.md` and `implementation_plan.md`. |
| Destination settings persist across restart without moving files | Verified | Tested in native Rust (`test_settings_persistence_and_no_filesystem_mutation`) and backend suite (`test_windows_settings_persistence_and_filesystem_immutability`). Settings saved to `settings.json` survive reload with identical values, reject invalid/nested paths, and perform zero filesystem moves or mutations on source/destination media. |
| Duplicate review remains non-destructive | Verified | Tested in native Rust (`review_and_export_preserve_media_and_exclusions`) and backend suite (`test_windows_duplicate_review_non_destructive`). Exact duplicate analysis identifies groups without modifying any files. Review-only exclusion flags and proposal export leave all files byte-identical with zero automatic deletion. |
| Collision handling and rollback restore media and sidecars | Verified | Tested in native Rust (`test_collision_handling_and_rollback_restores_media_and_sidecars`) and backend suite (`test_windows_collision_handling_and_rollback`). Pre-existing destination files and sidecars are untouched; colliding items are disambiguated with `_1` suffixes. Rollback reverses all moves, restoring media and sidecars (`.json` and `.supplemental-metadata.json`) back to source locations with verified SHA-256 match, leaving destinations clean. |
| People-search status never claims ready without a completed runtime-backed index | Verified | Tested in native Rust (`test_people_search_status_never_claims_ready_and_cleans_sidecars`) and backend suite (`test_windows_people_search_readiness_and_unapproved_runtime`). Setting disabled returns `disabled`; setting enabled returns `not_ready`. Even when an old derived SQLite database exists, it returns `not_ready` with 0 indexed media. Deleting index cleanly removes the database and WAL/SHM sidecars. |
| Results and follow-up tasks recorded in roadmap | Verified | Updated `implementation_plan.md` roadmap and linked evidence to issue #39. |

## Follow-up and roadmap alignment

- Keep issue #22 and #33 people search gated on feasibility and lack of an approved local face runtime.
- Captions support description editing and storage; automated generation remains deferred until a local vision model is vetted.
- With verification of migration, settings persistence, independent root scans, duplicate review, and collision rollback complete on Windows desktop, the acceptance criteria for [issue #39](https://github.com/KevinHozak/DreamCatcher/issues/39) are satisfied.

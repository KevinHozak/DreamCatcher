# Issue #18 Implementation Plan: Indexed Pictures and Videos Inventory

## Dependency Note

Issue #18 depends on #17. This branch starts from `origin/main` as required by the work workflow. The final implementation should consume the merged settings contract from #17, or be rebased onto that branch if #17 is still under review.

## Acceptance Criteria Mapping

1. Configured Pictures and Videos roots can be scanned independently.
2. Repeated scans do not duplicate records and handle moved or removed files safely.
3. Statistics come from the canonical inventory and distinguish pictures from videos.
4. Unsupported, unreadable, and malformed files are reported without aborting the scan.
5. Tests cover incremental scans, stale records, cancellation, and representative metadata.

## Current State

- `backend/core/scanner.py` and `frontend/src-tauri/src/scanner.rs` perform filesystem scans but return transient lists only.
- Existing records include paths, extensions, media type, size, timestamps, sidecars, and GPS where available.
- Existing scans skip protected directories and support a month filter, but do not persist scan provenance or stale-record state.
- The frontend currently consumes scan results directly as triage state.

## Proposed Architecture

- Define a canonical media-record schema with a stable identity, canonical path, media type, byte size, timestamps, metadata provenance, scan ID, and availability state.
- Add a local persistence layer appropriate to the desktop runtime, keeping the canonical inventory separate from thumbnail/search/face-derived caches.
- Implement independent Pictures and Videos scan jobs with progress, cancellation, skipped-file diagnostics, and last-successful-scan metadata.
- Reconcile each scan by upserting discovered records, marking missing records stale, and avoiding duplicate rows when a scan repeats.
- Provide statistics for picture/video counts, total bytes, extensions, date ranges, stale records, skipped files, and scan health.
- Expose a query/API contract that later library browsing and filters can consume without rescanning the filesystem.

## Proposed File Changes

- Add inventory models and persistence under `frontend/src-tauri/src/` for the native runtime.
- Add matching Python inventory/API modules for development mode.
- Extend scanner results with stable identity, provenance, and diagnostics without changing existing triage behavior.
- Add frontend API types and scan-status/statistics state.
- Add focused Rust and Python tests using temporary media trees, malformed files, repeated scans, removed files, and cancellation.

## Safety and Performance

- Never delete user files as part of indexing.
- Treat unreadable files as diagnostics, not fatal scan errors.
- Keep canonical paths normalized and avoid following protected/generated directories.
- Bound memory and UI updates for large libraries; do not require thumbnails during indexing.
- Make cancellation leave the last completed inventory intact and mark the interrupted run clearly.

## Verification Plan

- Run Rust inventory tests and existing media tests.
- Run Python scanner/API tests and an incremental reconciliation test suite.
- Run frontend TypeScript and lint checks.
- Validate statistics against a fixture tree containing pictures, videos, sidecars, malformed files, duplicates, and removed files.
- Run `git diff --check` and inspect the final diff for unrelated changes.

# Issue #17 Implementation Plan: Folder Settings

## Decisions

- Use native Tauri folder pickers, with text fields retained for direct path editing.
- Allow Pictures and Videos destinations to remain unset independently.

## Acceptance Criteria Mapping

1. Users can view and edit independent Pictures and Videos destination folders in one settings area.
2. Existing folder settings remain available and represented consistently.
3. Invalid, inaccessible, duplicate, or unsafe paths are rejected with actionable feedback.
4. Settings persist across app restarts and have focused backend/frontend tests.
5. The contract documents platform and path assumptions for the Tauri desktop runtime.

## Current State

- `frontend/src/App.tsx` owns a hard-coded `sourceDir` value and has no settings screen.
- `frontend/src/services/api.ts` contains the HTTP client boundary for the Python backend.
- `backend/main.py` currently exposes scan, execute, system-status, and media routes only.
- `frontend/src-tauri/` is the production desktop boundary, while the Python backend remains the current development/API path.

## Proposed File Changes

- Add a backend settings model and persistence module with explicit `source_dir`, `pictures_dir`, and `videos_dir` values.
- Add settings read/write routes with shared path validation and actionable error responses.
- Add frontend API types and settings calls in `frontend/src/services/api.ts`.
- Add a settings view/component and integrate navigation from the existing cockpit header.
- Replace the hard-coded ingest path with the persisted source-folder setting while preserving the current scan flow.
- Add focused Python API tests and frontend type/build validation.
- Document path validation, persistence location, unset values, and Tauri/runtime assumptions.

## Validation Rules

- Paths must be absolute and syntactically valid for the host platform.
- Existing paths must be directories and readable; creation of missing destinations should be an explicit later decision.
- Pictures and Videos destinations must not resolve to the same directory.
- One configured destination must not contain the other, or vice versa.
- Settings writes must not move files or alter the filesystem beyond persistence.

## Verification Plan

- Run backend tests, including invalid paths, duplicate paths, nested paths, persistence, and restart/read-back behavior.
- Run frontend TypeScript/build checks.
- Exercise the settings API and UI with valid, unavailable, duplicate, and nested paths.
- Run `git diff --check` and inspect the final diff for unrelated changes.

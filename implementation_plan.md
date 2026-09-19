# Issue #19 Implementation Plan: Configured Destination Routing

## Dependency

Issue #19 depends on the persisted folder-settings contract from #17 and consumes the inventory/media boundaries from #18 where available.

## Acceptance Criteria Mapping

1. Picture albums route to the configured Pictures destination and video albums route to Videos.
2. Resolved destinations are visible before execution.
3. Unsafe or ambiguous destinations block execution before file changes.
4. Collision and rollback behavior remains intact.
5. Tests cover mixed media, repeated execution, collisions, and partial failures.

## Implemented Design

- Keep existing source-relative `Pictures` and `Videos` folders as the fallback when a destination is unset.
- Accept configured destinations through both the Python API and Tauri command boundary.
- Validate configured destinations before processing any decision: existing directory, not the source root, and no overlap between Pictures and Videos.
- Preserve the existing unique-name collision strategy and ledger-based rollback.
- Return resolved destinations in execution results and show them in the final execution preview.

## Target Files

- `backend/core/executor.py` and `backend/api/routes_execute.py`
- `frontend/src-tauri/src/executor.rs` and `frontend/src-tauri/src/lib.rs`
- `frontend/src/services/api.ts`, `frontend/src/App.tsx`, and `frontend/src/components/ExecutionModal.tsx`
- `backend/tests/test_destination_routing.py`

## Verification Plan

- Test configured picture/video routing and fallback behavior.
- Test overlap and invalid-destination rejection before file movement.
- Run existing collision and rollback tests.
- Run Rust tests, TypeScript compilation, frontend lint, Python compilation, and `git diff --check`.

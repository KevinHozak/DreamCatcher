# Issue #21 Implementation Plan: Duplicate Detection and Cleanup Proposals

## Dependency

Issue #21 consumes the durable inventory from #18 and the read-only library/query surface from #20.

## Acceptance Criteria Mapping

1. Exact duplicates are identified using stable file fingerprints.
2. Candidate groups show paths, sizes, timestamps, and matching evidence.
3. Users can exclude candidates; no file is deleted automatically.
4. Cleanup proposals integrate with preview/rollback safety if execution is added later.
5. Performance and storage costs are measured on representative libraries.

## Proposed Design

- Add a background-capable duplicate analysis job over available inventory records.
- Use a two-stage strategy: exact content fingerprints first, then optional perceptual signatures for visually similar pictures and media fingerprints for videos.
- Store duplicate groups and evidence as derived data separate from the canonical inventory.
- Present groups in a review surface with confidence/evidence labels, file metadata, and an explicit keep/exclude choice.
- Generate an exportable cleanup proposal or executor-compatible action plan, but never delete automatically.

## Safety and Privacy

- Exact duplicate analysis may read file content but must not modify user files.
- Perceptual analysis must be opt-in if it requires expensive decoding or derived embeddings.
- Preserve the original inventory, source paths, sidecar relationships, and selected keepers.
- Require a separate explicit confirmation and existing rollback/ledger flow before any future cleanup execution.

## Target Files

- Add duplicate analysis and derived-group persistence under `backend/core/` and `frontend/src-tauri/src/`.
- Add API/Tauri commands for starting analysis, reading paginated groups, excluding candidates, and exporting proposals.
- Add a review UI under `frontend/src/components/` reachable from the Media Library.
- Extend inventory records only with references to derived duplicate groups, not duplicate-owned state.

## Verification Plan

- Test exact duplicates with identical content and different filenames/paths.
- Test near-duplicates separately from exact matches and preserve confidence evidence.
- Test exclusions, stale records, sidecars, large-file streaming hashes, and cancellation.
- Verify no analysis path deletes or moves files.
- Measure hashing/index storage cost on representative fixture sizes.
- Run Rust tests, Python tests, TypeScript compilation, frontend lint, and `git diff --check`.

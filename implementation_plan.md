# Issue #22 Implementation Plan: Opt-In Local People Search

## Scope

Issue #22 adds a privacy-preserving people-search capability for the indexed
picture library. The feature must remain local-first, explicitly opt-in, and
separate from the canonical media inventory. It is a feasibility-first
milestone: no facial-recognition implementation should be accepted until the
model, runtime, hardware, accuracy, licensing, storage, and maintenance costs
have been documented and approved in the product surface or project docs.

## Dependency and boundaries

- Parent epic: #16 Media Library
- Depends on: #20 Media Library foundation
- Reuse the canonical inventory and library query surface from #18/#20.
- Reuse the existing settings surface for the opt-in and derived-data controls.
- Do not modify, move, rename, delete, or upload media as part of indexing.
- Do not assign a person's identity automatically. Search must use reviewed
  labels attached to locally generated face records.

## Feasibility decision (required before implementation)

Document a written decision covering:

1. Candidate local model(s), runtime(s), supported OS/CPU/GPU paths, model
   download/packaging strategy, and minimum hardware expectations.
2. Recognition quality, confidence thresholds, false-match and missed-match
   behavior, difficult-media limitations, and expected indexing performance.
3. Model/runtime licenses, redistribution obligations, data retention, and
   whether any dependency can make a network request.
4. Derived storage size, rebuild time, cancellation behavior, migration
   strategy, and support burden.
5. Whether to implement now behind a feature flag or explicitly defer until a
   safe local runtime is available.

The decision must be visible in the issue-linked documentation and reflected
in the UI's limitations/help text. A deferral is an acceptable outcome.

## Product and data design

- Add an explicit `people_search_enabled` setting whose default is disabled.
- Add controls for starting, cancelling, pausing/resuming if supported,
  rebuilding, and deleting the derived face index independently of media.
- Store face detections, embeddings, processing metadata, and reviewed person
  labels in a separate derived store, never in the canonical inventory tables
  or media files.
- Track model/runtime version, index version, source media identity, status,
  confidence, and timestamps so stale or incompatible records can be rebuilt.
- Make deletion of the derived index complete and verifiable; retain only
  ordinary media inventory metadata unless the user separately removes it.
- Process pictures only after opt-in and allow cancellation without leaving a
  falsely-complete index.

## Search and review behavior

- Add a People section to the media-library experience only when people search
  is enabled and a usable local index exists.
- Search by reviewed person label, with clear distinction between reviewed
  matches, unreviewed face clusters, and low-confidence candidates.
- Provide review, rename, merge, split, reject, and delete operations for
  person labels/clusters without changing media files.
- Show the index status, last run, model/runtime version, coverage, and any
  skipped or failed items.
- Keep existing path, year, date, type, size, and duplicate filters available
  alongside people filters where the query surface supports it.

## Privacy and safety requirements

- Default behavior must be no face processing and no face-derived data.
- Explain locally processed data, retention, limitations, and deletion before
  opt-in; require affirmative confirmation.
- Do not send images, embeddings, labels, or diagnostics to remote services by
  default. Fail closed if a selected runtime requires network access.
- Never expose embeddings as ordinary user-facing search text or export them
  without a separately designed privacy review.
- Treat false matches as expected; require human review before a person label
  becomes searchable.
- Preserve the existing no-automatic-cleanup rule: people indexing cannot
  delete or move duplicate/media files.

## Target implementation areas

- `backend/core/`: derived people-index schema, opt-in guards, job lifecycle,
  cancellation, rebuild/delete, and reviewed-label queries.
- `backend/api/`: status, lifecycle, review, and people-search endpoints with
  explicit disabled/not-ready responses.
- `frontend/src/services/api.ts`: settings, job status, review, and search
  contracts with Tauri/backend parity.
- `frontend/src/components/`: settings controls, index-status/review surface,
  and people-filter integration in the media library.
- `frontend/src-tauri/src/`: durable local settings, derived-index commands,
  and deletion/rebuild guarantees if native execution is selected.
- `docs/` or issue-linked project documentation: feasibility decision,
  privacy/retention statement, support matrix, and operator guidance.

## Verification plan

- Verify a fresh install has people search disabled and no face-derived store.
- Verify opt-in is explicit, cancellable, repeatable, and local-only.
- Verify disabled, unavailable-runtime, cancelled, failed, stale, and
  rebuild-in-progress states are safe and understandable.
- Verify reviewed labels are the only identity-searchable values.
- Verify deleting/rebuilding the derived index leaves canonical inventory and
  media files untouched.
- Test review operations, filters, pagination, stale source records, and
  model/index version changes.
- Add focused backend/Rust tests, TypeScript compilation, frontend lint,
  `git diff --check`, and document the known build-environment limitations.

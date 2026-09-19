# Issue #20 Implementation Plan: Media Library Browsing, Search, and Filters

## Dependency

Issue #20 consumes the persisted folder settings from #17 and the durable inventory/statistics contract from #18.

## Acceptance Criteria Mapping

1. Users can browse Pictures and Videos independently.
2. Counts, total size, date range, and last scan status are visible.
3. Search and year/date filters query the inventory correctly.
4. Filters combine and clear predictably.
5. Large inventories remain usable through bounded loading or virtualization.

## Proposed Design

- Add a dedicated Media Library surface reachable from the main cockpit navigation.
- Present Pictures and Videos as independent tabs with summary cards for item count, total bytes, date range, stale records, and last scan.
- Add a backend/Tauri query contract with pagination, text search, media type, year/date range, extension, size range, and state filters.
- Keep filtering in the inventory layer so the UI does not load the entire media collection or rescan the filesystem.
- Add explicit loading, scanning, stale, empty, unavailable, and error states.
- Let users inspect the source path and metadata for a selected result without moving or deleting anything.

## Target Files

- Add `MediaLibraryView` and supporting result/stat cards under `frontend/src/components/`.
- Extend `frontend/src/App.tsx` navigation and `frontend/src/services/api.ts` query types.
- Add paginated inventory query functions to `backend/core/inventory.py` and `backend/api/routes_inventory.py`.
- Add matching Tauri inventory query command and bounded result serialization.
- Add Python and Rust tests for combined filters, year boundaries, pagination, stale state, and empty results.

## Safety and Performance

- Library browsing is read-only and must never invoke executor operations.
- Return only bounded result pages; do not serialize the entire inventory for each query.
- Treat stale records as visible but clearly marked until the next successful scan.
- Escape or parameterize all search/filter inputs at the inventory query boundary.
- Preserve the canonical inventory as the source of truth; thumbnails and future face indexes remain derived caches.

## Verification Plan

- Test query correctness for Pictures/Videos, text search, year/date ranges, extensions, sizes, and combined filters.
- Test pagination stability and stale/unavailable states.
- Run Rust tests, Python inventory/API tests, TypeScript compilation, frontend lint, and `git diff --check`.

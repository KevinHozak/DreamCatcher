# DreamCatcher roadmap and implementation status

This file is the repository's concise roadmap and the local reconciliation
point for the GitHub issue list. It records repository evidence as of 2026-09-28;
confirm GitHub state before closing or reprioritizing issues.

## Product direction

Keep the Windows Tauri desktop app as the primary user experience. Treat the
Python/FastAPI backend as a development and API implementation until native
feature parity is deliberately delivered. Preserve local-first behavior,
explicit user approval for file operations, and reversible changes.

## Current delivery

| Area | Verified repository state | Remaining gap |
| --- | --- | --- |
| Triage cockpit | Native scan, classification, review, clustering, execution, rollback | Confirm packaged build and real-library performance on supported Windows installs |
| Folder settings | Source and independent picture/video destinations persist in native settings | Improve validation/error feedback if gaps remain in issue #17 |
| Inventory | Native and Python paths use the shared SQLite database; native migration from legacy JSON is present | Verify migrations against representative real installations and failure recovery |
| Library | Native and Python paths scan, count, page, and filter the SQLite inventory | Native query does not yet implement every advertised filter (notably person/date-range), and native response omits caption fields |
| Duplicate review | Native duplicate analysis and exclusion/review surface exist | Confirm exact/perceptual behavior, scale, and safe next action; no automatic deletion |
| Captions | Python store imports descriptions and supports editing; processing has no configured generator | Tauri does not expose caption fields/editing; real local generation/progress/cancellation are not implemented |
| People search | Opt-in setting, isolated record store, review/search API foundation and delete control exist | No vetted face runtime; actual native recognition/indexing is absent |
| Video previews | Generic fallback exists | Representative-frame extraction remains a candidate enhancement (issue #30) |

The README and `docs/` describe user-visible boundaries. Issue completion
should reflect the actual supported runtime, not merely the presence of a
backend endpoint or storage schema.

## Recommended next steps

### P0 — Verify and align the released desktop contract

1. **Reconcile GitHub issue state with the code and release evidence.** In
   particular, review #16 through #22 and #31 through #33. Do not mark people
   search complete: the feasibility gate permits a deferral, but the full
   runtime/index/search acceptance criteria are not met. Correct epic checklists
   or titles where they imply the full feature shipped.
2. **Choose and document the supported runtime boundary.** Decide whether
   Python is a supported companion runtime or development-only. The README now
   states the observed split; next remove misleading parity assumptions from
   the issue acceptance criteria and release checklist.
3. **Harden media-library parity and readiness states.** Add or explicitly
   defer native caption metadata/editing and date-range filtering. Keep person
   filtering hidden unless a usable index exists, and keep the face runtime
   disabled and unconfigured.
4. **Run a release verification pass** on a clean Windows environment and a
   representative Takeout: migration from legacy JSON, scans, filters,
   duplicates, settings persistence, file collision handling, and rollback.
   Record results and supported limitations before calling the media-library
   epic done. The partial preflight for issue #39 is recorded in
   [`docs/release-verification-issue-39.md`](docs/release-verification-issue-39.md);
   it built the 1.8.4 x64 installer and passed seven native unit tests, but did
   not run the packaged UI workflows or a representative media fixture. Keep
   release sign-off open until the interactive checks in that record are done.

### P1 — Finish useful non-biometric library workflows

5. Deliver caption metadata/editing consistently in Tauri, or mark captions as
   Python-only in UI and tracking. For generated descriptions, first select a
   local model, document license/resources/video sampling, then implement
   visible progress, cancellation, retry, and stale-result behavior.
6. Measure duplicate matching quality/performance on real libraries; retain
   review-only behavior and do not add automatic deletion.
7. Decide whether video representative-frame thumbnails justify their codec,
   packaging, and test burden; issue #30 is a reasonable optional follow-up.

### P2 — Revisit people search only after a concrete feasibility decision

8. Evaluate candidates for local execution, redistribution/license, hardware
   support, accuracy, index size, rebuild time, and deletion. Update
   `docs/people-search-feasibility.md` with evidence before enabling indexing.
9. If no candidate passes those gates, keep people search explicitly deferred
   and remove UI language that implies an active usable search feature. If one
   does pass, make runtime setup and limitations explicit and build the native
   lifecycle/review experience before calling it complete.

## Issue-to-repository reconciliation

The GitHub project is the canonical task list; this section is a local summary,
not a substitute for updating issue bodies/statuses after verification.

| Issue(s) | Topic | Local evidence and recommendation |
| --- | --- | --- |
| #16–#20 | Media library epic, settings, inventory, routing, browsing | Main surfaces and native SQLite path exist. Verify criteria individually, especially destination routing, validation, migration backup/recovery, filter completeness, and packaged behavior. |
| #21 | Duplicate detection/review | Native and Python duplicate modules/UI exist. Verify confidence/explanation and performance criteria before closing. |
| #22 | Opt-in people search | Feasibility document records deferral; storage/review foundations exist, but no face runtime. Rewrite issue as feasibility-gated/deferred or leave implementation acceptance open. |
| #30 | Video frame thumbnails | Not part of current roadmap critical path; decide based on user value and codec support. |
| #31 | Shared SQLite inventory | Native module now uses SQLite with legacy JSON migration; verify backup and cross-runtime behavior from a built application before closing. |
| #32 | Captions/descriptions | Python implementation and docs exist; native parity and actual generation/progress are incomplete. Scope to clear deliverable(s). |
| #33 | Face records and reviewed people | Python derived-store/review foundations exist; native code only exposes file status/deletion. Keep acceptance open until runtime and native user flow exist, or explicitly defer. |

## Roadmap maintenance rules

- Keep one source of truth per feature: GitHub issues for actionable work,
  `docs/` for data/privacy/runtime contracts, this file for cross-issue status.
- Link changes to verified commits, tests, or manual release evidence.
- Separate Python and Tauri acceptance criteria whenever behavior differs.
- Keep date, scope, and status claims evidence-backed; use “deferred” or
  “not implemented” where a schema/API alone could otherwise sound complete.
- Update this roadmap after issue/PR merges and verify the GitHub read-back.

# Issue #51 implementation plan

User review required: none before implementing the authorized review-only benchmark scope.

- Record native analysis, SQLite fixture indexing, review pagination, exclusion, export, and process memory metrics at 5,000 and 50,000 synthetic files.
- Dispatch native duplicate work on blocking workers and serialize review-store operations.
- Bound rendered groups and members; export every group as a review-only JSON download.
- Verify byte-for-byte media preservation after analysis, exclusions, export, and reanalysis.
- Run native regression tests, scale benchmarks, frontend build, and lint. Document synthetic-fixture and interactive-UI limits.

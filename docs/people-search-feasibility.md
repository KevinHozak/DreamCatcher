# Local People Search Feasibility Decision

Status: deferred; no vetted local face runtime is selected or shipped

## Decision

DreamCatcher will not ship a facial-recognition model or download one silently
as part of this milestone. The application has a privacy boundary, opt-in
setting, and pieces of a derived-record/review contract, but indexing is not
implemented. The Python API's index-start endpoint returns a not-ready conflict;
the native app has no recognition/indexing workflow. An existing derived
database must not be treated as proof that a usable index exists.

## Required evaluation before enabling indexing

- A model and runtime that execute entirely on the user's device without
  sending media, embeddings, labels, or diagnostics to a remote service
- Supported Windows CPU/GPU paths and a documented minimum hardware profile
- Model accuracy and known false-match/missed-match behavior on varied image
  sizes, lighting, occlusion, groups, and video frames
- Model/runtime license compatibility and a safe packaging or first-run
  installation strategy
- Expected index size, processing time, cancellation behavior, and rebuild
  behavior for a representative library
- A retention and deletion guarantee for face records, embeddings, clusters,
  and reviewed labels

## Current safety contract

- `people_search_enabled` defaults to `false`
- No face-derived database is created while the setting is disabled
- Enabling the setting records an explicit user choice but does not start
  processing while the runtime is not ready
- The derived people index has a separate storage boundary and can be deleted
  without modifying canonical inventory or media files
- Person identity search must use reviewed labels only; automatic identity
  assignment is out of scope
- Remote processing is not an acceptable default or fallback
- The setting is consent to a future capability, not consent to start
  processing; no current runtime can process faces
- Only a completed, runtime-backed index may be shown as ready. Database/schema
  existence alone is insufficient evidence of indexed media

## Future implementation gate

The next implementation may add local indexing only after this document is
updated with the selected runtime, license, support matrix, measured quality,
performance results, and a user-facing explanation of limitations. Until then,
the UI must report that indexing is unavailable/not ready. Keep the feature
disabled by default and do not describe the current release as providing
functional face recognition or indexing.

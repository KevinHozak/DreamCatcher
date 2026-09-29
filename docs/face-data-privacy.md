# Face-derived data: local storage and retention

The Python people-search record layer can keep face-derived records in a
separate local SQLite database (`people-index.sqlite3`). The canonical media
inventory never stores regions, person labels, embeddings, or model output.
The feature is disabled by default. No recognition runtime is shipped, so the
current app cannot perform face indexing; the opt-in records a preference and
does not initiate face processing.

Only reviewed person labels are searchable. New detections are candidates and
must be confirmed by a person before they can affect Media Library results.
Rejected and ignored detections remain explicit review decisions so a future
reprocess does not silently recreate a known false match. Normalized regions,
confidence, source, model version, runtime version, and timestamps are retained
for provenance and review. Embeddings, when a vetted local runtime is added,
are stored in a dedicated table and are not returned by ordinary queries.

The current release provides some Python storage and review operations but
does not ship a face-recognition runtime or a complete native review workflow.
It cannot infer identities or upload media, regions, names, or embeddings.
Deleting the Python derived index removes its SQLite database and journal
sidecars while leaving media files and canonical inventory untouched. The
native delete command removes its index file; it does not provide processing or
review. Rebuilding must not be considered available until a runtime and job
lifecycle are implemented and verified.

Face matching is imperfect: false matches, missed faces, poor lighting,
occlusion, aging, duplicate media, and model/runtime changes are expected.
Users should verify every reviewed association and should treat retention of
face-derived data as sensitive local information.

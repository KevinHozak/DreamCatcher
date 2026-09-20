# Face-derived data: local storage and retention

DreamCatcher keeps face-derived records in a separate local SQLite database
(`people-index.sqlite3`). The canonical media inventory never stores regions,
person labels, embeddings, or model output. The feature is disabled by default
and no face processing occurs until the user explicitly enables people search.

Only reviewed person labels are searchable. New detections are candidates and
must be confirmed by a person before they can affect Media Library results.
Rejected and ignored detections remain explicit review decisions so a future
reprocess does not silently recreate a known false match. Normalized regions,
confidence, source, model version, runtime version, and timestamps are retained
for provenance and review. Embeddings, when a vetted local runtime is added,
are stored in a dedicated table and are not returned by ordinary queries.

The current release provides the storage and review boundary but does not ship
a face-recognition runtime. It therefore cannot infer identities or upload
media, regions, names, or embeddings. Delete derived index removes the SQLite
database and its SQLite journal sidecars while leaving media files and the
canonical inventory untouched. Rebuilding is safe to repeat because detection
identity is deterministic for a media identity, region, and model version, and
human review state is never overwritten by reprocessing.

Face matching is imperfect: false matches, missed faces, poor lighting,
occlusion, aging, duplicate media, and model/runtime changes are expected.
Users should verify every reviewed association and should treat retention of
face-derived data as sensitive local information.

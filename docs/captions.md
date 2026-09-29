# Media captions and descriptions

DreamCatcher stores imported and generated descriptions in the `media_captions`
table beside, but separate from, the canonical `media_inventory` table. The
original media and its embedded metadata are never rewritten by caption
processing.

## Provenance and precedence

Descriptions are imported during inventory reconciliation. User-authored text
has the highest precedence and is never replaced by a later scan. When no user
text exists, a matching Google Takeout JSON sidecar description is preferred,
followed by the JPEG/TIFF EXIF `ImageDescription` field. Each stored value has
`source`, `status`, `source_revision`, and update timestamps.

The supported sources are `sidecar`, `embedded`, `user`, and `generated`. A
description is editable through the Media Library API without changing the
sidecar or media file.

## Processing and privacy

Processing is resumable at the item level. Items with user, embedded, or
sidecar descriptions are skipped unless a rebuild is explicitly requested.
Failures remain marked `failed` with an error so a later retry can safely pick
them up. Cancellation stops before the next item and does not mark it complete.

The default API does not configure a captioning model or send media remotely.
Generated captions must come from an explicitly selected local runtime. They
can be inaccurate, incomplete, or sensitive and should be reviewed before
being treated as authoritative. Generic captioning never stores person
identities; face embeddings remain in the separate people-search store.

## Video behavior

The inventory and caption store identify videos as `media_type=video`. A local
video caption runtime must document its sampling strategy, such as bounded
keyframes at a fixed interval, maximum duration, and resource limits, and must
retain the source media identity in the generated record. The current API
returns a clear unavailable-runtime failure until such a runtime is configured.

The Python API endpoints are `GET /api/captions`, `PUT
/api/captions/{identity}`, and `POST /api/captions/process`. The process endpoint
currently has no configured generator, so generation attempts report an
unavailable-runtime failure. The native Tauri library does not yet expose
caption fields, editing, or processing; the frontend reports those operations
as unavailable in native mode. None of the Python endpoints modifies, moves,
renames, or deletes media files.

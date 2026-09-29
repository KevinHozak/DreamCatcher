# Canonical inventory storage

DreamCatcher uses one local SQLite database for indexed media metadata:

`%LOCALAPPDATA%\\DreamCatcher\\inventory.sqlite3` on Windows, or
`$XDG_DATA_HOME/DreamCatcher/inventory.sqlite3` on other platforms. The
`DREAMCATCHER_INVENTORY_PATH` environment variable may override this location
for tests and controlled deployments.

The Python backend and native Tauri runtime use the same database path and
canonical media tables: `media_inventory`, `inventory_scans`,
`inventory_diagnostics`, and `inventory_meta`. The Python backend also creates a
related `media_captions` table in this database. The native inventory query
currently does not return caption fields. Duplicate review uses derived data;
people-search data remains in a separate derived store. Neither changes
canonical media records.

## Migration and recovery

When the native runtime first opens a database with no schema marker, it looks
for the legacy `inventory.json`, parses it before writing records, and copies it
to `inventory.json.migrated-<timestamp>.bak`. Migration uses `INSERT OR IGNORE`
and is therefore safe to repeat. A malformed JSON file or a database with a
newer schema version stops startup of that inventory operation with an explicit
error. Media files are never moved or deleted by migration.

Keep the generated backup until the database has been validated. Recovery is a
manual, additive operation: preserve the SQLite file, restore or inspect the
backup, and rerun the migration only against a new database after confirming
the desired source data. SQLite WAL mode and a ten-second busy timeout support
multiple readers and serialized local writers. Test the actual packaged
desktop/Python combination before promising concurrent access under sustained
writes.

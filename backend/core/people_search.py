"""Privacy-preserving people-search foundation.

Face processing is intentionally not bundled until a local model/runtime has
passed the feasibility and licensing review. This module owns the separate
derived-store boundary so the eventual implementation cannot contaminate the
canonical media inventory.
"""

import os
import sqlite3
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional


def default_people_index_path() -> Path:
    configured = os.environ.get("DREAMCATCHER_PEOPLE_INDEX_PATH")
    if configured:
        return Path(configured)
    if os.name == "nt":
        root = Path(os.environ.get("LOCALAPPDATA", Path.home() / "AppData/Local"))
    else:
        root = Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local/share"))
    return root / "DreamCatcher" / "people-index.sqlite3"


def _utc_now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _connect(path: Path) -> sqlite3.Connection:
    path.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(path)
    conn.row_factory = sqlite3.Row
    conn.executescript(
        """
        CREATE TABLE IF NOT EXISTS people_index_meta (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS face_records (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            media_identity TEXT NOT NULL,
            media_path TEXT NOT NULL,
            embedding BLOB,
            confidence REAL,
            reviewed_person_id INTEGER,
            model_version TEXT NOT NULL,
            created_at TEXT NOT NULL,
            UNIQUE(media_identity, id)
        );
        CREATE TABLE IF NOT EXISTS people (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            label TEXT NOT NULL,
            reviewed INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_face_media_identity ON face_records(media_identity);
        CREATE INDEX IF NOT EXISTS idx_face_person ON face_records(reviewed_person_id);
        CREATE INDEX IF NOT EXISTS idx_people_label ON people(label);
        """
    )
    return conn


def people_search_status(enabled: bool, index_path: Optional[Path] = None) -> dict:
    path = index_path or default_people_index_path()
    if not enabled:
        return {
            "enabled": False,
            "state": "disabled",
            "message": "People search is disabled. No face processing or face-derived data is created.",
            "index_exists": path.exists(),
            "indexed_media": 0,
            "reviewed_people": 0,
            "last_run": None,
        }
    if not path.exists():
        return {
            "enabled": True,
            "state": "not_ready",
            "message": "People search is enabled, but no vetted local face runtime is configured yet.",
            "index_exists": False,
            "indexed_media": 0,
            "reviewed_people": 0,
            "last_run": None,
        }
    conn = _connect(path)
    try:
        indexed = conn.execute("SELECT COUNT(DISTINCT media_identity) FROM face_records").fetchone()[0]
        reviewed = conn.execute("SELECT COUNT(*) FROM people WHERE reviewed=1").fetchone()[0]
        last_run = conn.execute("SELECT value FROM people_index_meta WHERE key='last_run'").fetchone()
        return {
            "enabled": True,
            "state": "ready",
            "message": "People search is available for reviewed local labels.",
            "index_exists": True,
            "indexed_media": indexed,
            "reviewed_people": reviewed,
            "last_run": last_run[0] if last_run else None,
        }
    finally:
        conn.close()


def delete_people_search_index(index_path: Optional[Path] = None) -> dict:
    path = index_path or default_people_index_path()
    if path.exists():
        path.unlink()
    return {"deleted": True}

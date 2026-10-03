"""Local, privacy-separated records for reviewed people and face analysis."""

import os
import sqlite3
from datetime import datetime, timezone
from pathlib import Path
from typing import Iterable, Optional

SCHEMA_VERSION = 1
VALID_REVIEW_STATES = {"candidate", "confirmed", "rejected", "ignored"}


def default_people_index_path() -> Path:
    configured = os.environ.get("DREAMCATCHER_PEOPLE_INDEX_PATH")
    if configured:
        return Path(configured)
    root = Path(os.environ.get("LOCALAPPDATA", Path.home() / "AppData/Local")) if os.name == "nt" else Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local/share"))
    return root / "DreamCatcher" / "people-index.sqlite3"


def _utc_now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _connect(path: Path) -> sqlite3.Connection:
    path.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(path)
    try:
        conn.row_factory = sqlite3.Row
        conn.execute("PRAGMA foreign_keys=ON")
        conn.executescript("""
            CREATE TABLE IF NOT EXISTS people_index_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS people (id INTEGER PRIMARY KEY AUTOINCREMENT, label TEXT NOT NULL, reviewed INTEGER NOT NULL DEFAULT 0 CHECK(reviewed IN (0,1)), created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
            CREATE UNIQUE INDEX IF NOT EXISTS idx_people_reviewed_label ON people(label) WHERE reviewed=1;
            CREATE TABLE IF NOT EXISTS face_clusters (id INTEGER PRIMARY KEY AUTOINCREMENT, cluster_key TEXT NOT NULL UNIQUE, status TEXT NOT NULL DEFAULT 'candidate' CHECK(status IN ('candidate','confirmed','rejected','ignored')), created_at TEXT NOT NULL, updated_at TEXT NOT NULL);
            CREATE TABLE IF NOT EXISTS face_detections (
                id INTEGER PRIMARY KEY AUTOINCREMENT, media_identity TEXT NOT NULL, media_path TEXT NOT NULL,
                region_left REAL NOT NULL, region_top REAL NOT NULL, region_right REAL NOT NULL, region_bottom REAL NOT NULL,
                confidence REAL, review_state TEXT NOT NULL DEFAULT 'candidate' CHECK(review_state IN ('candidate','confirmed','rejected','ignored')),
                cluster_id INTEGER REFERENCES face_clusters(id) ON DELETE SET NULL, person_id INTEGER REFERENCES people(id) ON DELETE SET NULL,
                source TEXT NOT NULL, model_version TEXT NOT NULL, runtime_version TEXT NOT NULL, detected_at TEXT NOT NULL, updated_at TEXT NOT NULL,
                UNIQUE(media_identity, region_left, region_top, region_right, region_bottom, model_version)
            );
            CREATE INDEX IF NOT EXISTS idx_detections_media ON face_detections(media_identity);
            CREATE INDEX IF NOT EXISTS idx_detections_person ON face_detections(person_id);
            CREATE TABLE IF NOT EXISTS face_embeddings (detection_id INTEGER PRIMARY KEY REFERENCES face_detections(id) ON DELETE CASCADE, embedding BLOB NOT NULL, created_at TEXT NOT NULL);
        """)
        conn.execute("INSERT OR REPLACE INTO people_index_meta(key,value) VALUES('schema_version',?)", (str(SCHEMA_VERSION),))
        return conn
    except Exception:
        conn.close()
        raise


def _validate_region(region: dict) -> tuple[float, float, float, float]:
    values = tuple(float(region[key]) for key in ("left", "top", "right", "bottom"))
    if not all(0 <= value <= 1 for value in values) or values[0] >= values[2] or values[1] >= values[3]:
        raise ValueError("Face region coordinates must be normalized and satisfy 0 <= left < right <= 1 and 0 <= top < bottom <= 1")
    return values


def upsert_detections(detections: Iterable[dict], index_path: Optional[Path] = None) -> dict:
    """Insert detections idempotently without overwriting human decisions."""
    conn = _connect(index_path or default_people_index_path())
    inserted = 0
    try:
        for item in detections:
            left, top, right, bottom = _validate_region(item["region"])
            now = _utc_now()
            existing = conn.execute("SELECT id FROM face_detections WHERE media_identity=? AND region_left=? AND region_top=? AND region_right=? AND region_bottom=? AND model_version=?", (item["media_identity"], left, top, right, bottom, item["model_version"])).fetchone()
            if existing:
                conn.execute("UPDATE face_detections SET media_path=?,confidence=?,source=?,runtime_version=?,detected_at=?,updated_at=? WHERE id=?", (item["media_path"], item.get("confidence"), item["source"], item["runtime_version"], item.get("detected_at", now), now, existing["id"]))
                continue
            conn.execute("INSERT INTO face_detections(media_identity,media_path,region_left,region_top,region_right,region_bottom,confidence,source,model_version,runtime_version,detected_at,updated_at) VALUES(?,?,?,?,?,?,?,?,?,?,?,?)", (item["media_identity"], item["media_path"], left, top, right, bottom, item.get("confidence"), item["source"], item["model_version"], item["runtime_version"], item.get("detected_at", now), now))
            inserted += 1
        conn.execute("INSERT OR REPLACE INTO people_index_meta(key,value) VALUES('last_run',?)", (_utc_now(),))
        conn.commit()
        return {"inserted": inserted, "total": conn.execute("SELECT COUNT(*) FROM face_detections").fetchone()[0]}
    finally:
        conn.close()


def create_person(label: str, index_path: Optional[Path] = None) -> dict:
    label = label.strip()
    if not label:
        raise ValueError("Person label is required")
    conn = _connect(index_path or default_people_index_path())
    try:
        now = _utc_now()
        person_id = conn.execute("INSERT INTO people(label,created_at,updated_at) VALUES(?,?,?)", (label, now, now)).lastrowid
        conn.commit()
        return {"id": person_id, "label": label, "reviewed": False}
    finally:
        conn.close()


def review_detection(detection_id: int, action: str, person_id: Optional[int] = None, index_path: Optional[Path] = None) -> dict:
    if action not in VALID_REVIEW_STATES:
        raise ValueError(f"Unsupported review state: {action}")
    conn = _connect(index_path or default_people_index_path())
    try:
        if not conn.execute("SELECT id FROM face_detections WHERE id=?", (detection_id,)).fetchone():
            raise ValueError("Face detection not found")
        if action == "confirmed":
            if person_id is None or not conn.execute("SELECT 1 FROM people WHERE id=?", (person_id,)).fetchone():
                raise ValueError("A valid person_id is required to confirm a detection")
            conn.execute("UPDATE people SET reviewed=1,updated_at=? WHERE id=?", (_utc_now(), person_id))
        else:
            person_id = None
        conn.execute("UPDATE face_detections SET review_state=?,person_id=?,updated_at=? WHERE id=?", (action, person_id, _utc_now(), detection_id))
        conn.commit()
        return {"id": detection_id, "review_state": action, "person_id": person_id}
    finally:
        conn.close()


def list_detections(media_identity: str, index_path: Optional[Path] = None) -> list[dict]:
    conn = _connect(index_path or default_people_index_path())
    try:
        rows = conn.execute("SELECT id,media_identity,media_path,region_left,region_top,region_right,region_bottom,confidence,review_state,cluster_id,person_id,source,model_version,runtime_version,detected_at,updated_at FROM face_detections WHERE media_identity=? ORDER BY id", (media_identity,)).fetchall()
        return [{**dict(row), "region": {"left": row["region_left"], "top": row["region_top"], "right": row["region_right"], "bottom": row["region_bottom"]}} for row in rows]
    finally:
        conn.close()


def rename_person(person_id: int, label: str, index_path: Optional[Path] = None) -> dict:
    label = label.strip()
    if not label:
        raise ValueError("Person label is required")
    conn = _connect(index_path or default_people_index_path())
    try:
        if not conn.execute("SELECT id FROM people WHERE id=?", (person_id,)).fetchone():
            raise ValueError("Person not found")
        conn.execute("UPDATE people SET label=?,updated_at=? WHERE id=?", (label, _utc_now(), person_id))
        conn.commit()
        return {"id": person_id, "label": label}
    finally:
        conn.close()


def delete_detection(detection_id: int, index_path: Optional[Path] = None) -> dict:
    conn = _connect(index_path or default_people_index_path())
    try:
        deleted = conn.execute("DELETE FROM face_detections WHERE id=?", (detection_id,)).rowcount
        conn.commit()
        return {"deleted": bool(deleted)}
    finally:
        conn.close()


def query_reviewed_people(search: Optional[str] = None, index_path: Optional[Path] = None) -> list[dict]:
    conn = _connect(index_path or default_people_index_path())
    try:
        params: list[object] = []
        clause = "WHERE p.reviewed=1"
        if search and search.strip():
            clause += " AND LOWER(p.label) LIKE ?"
            params.append(f"%{search.strip().lower()}%")
        rows = conn.execute(f"SELECT p.id,p.label,COUNT(DISTINCT d.media_identity) AS media_count FROM people p LEFT JOIN face_detections d ON d.person_id=p.id AND d.review_state='confirmed' {clause} GROUP BY p.id ORDER BY p.label", params).fetchall()
        return [dict(row) for row in rows]
    finally:
        conn.close()


def reviewed_media_identities(label: Optional[str], index_path: Optional[Path] = None) -> set[str]:
    if not label or not label.strip():
        return set()
    conn = _connect(index_path or default_people_index_path())
    try:
        rows = conn.execute("SELECT DISTINCT d.media_identity FROM face_detections d JOIN people p ON p.id=d.person_id WHERE d.review_state='confirmed' AND p.reviewed=1 AND LOWER(p.label)=LOWER(?)", (label.strip(),)).fetchall()
        return {row[0] for row in rows}
    finally:
        conn.close()


def people_search_status(enabled: bool, index_path: Optional[Path] = None) -> dict:
    path = index_path or default_people_index_path()
    if not enabled:
        return {"enabled": False, "state": "disabled", "message": "People search is disabled. No face processing or face-derived data is created.", "index_exists": path.exists(), "indexed_media": 0, "reviewed_people": 0, "last_run": None}
    if not path.exists():
        return {"enabled": True, "state": "not_ready", "message": "People search is enabled, but no approved local face runtime has produced an index.", "index_exists": False, "indexed_media": 0, "reviewed_people": 0, "last_run": None}
    try:
        conn = _connect(path)
        try:
            # Until a vetted runtime and indexing lifecycle exist, a database file
            # (including manually supplied detections) must not claim readiness.
            return {"enabled": True, "state": "not_ready", "message": "People search is enabled, but indexing is unavailable because no vetted local face runtime is configured. The existing derived database is not a usable index.", "index_exists": True, "indexed_media": 0, "reviewed_people": 0, "last_run": None}
        finally:
            conn.close()
    except (sqlite3.DatabaseError, sqlite3.OperationalError):
        return {"enabled": True, "state": "not_ready", "message": "People search is enabled, but indexing is unavailable because no vetted local face runtime is configured. The existing derived database is not a usable index.", "index_exists": True, "indexed_media": 0, "reviewed_people": 0, "last_run": None}


def delete_people_search_index(index_path: Optional[Path] = None) -> dict:
    path = index_path or default_people_index_path()
    for suffix in ("", "-wal", "-shm"):
        candidate = Path(f"{path}{suffix}")
        if candidate.exists():
            candidate.unlink()
    return {"deleted": True}

"""Caption metadata and local processing for the canonical media inventory.

Captions are deliberately stored separately from the media files and from the
derived people-search store.  The module is synchronous so callers can supply
their own job runner and cancellation policy (the API currently uses the
bounded batch helper).
"""

import hashlib
import json
import sqlite3
from datetime import datetime, timezone
from pathlib import Path
from typing import Callable, Optional

from core.scanner import PHOTO_EXTS, find_json_sidecar

SOURCES = {"embedded", "sidecar", "user", "generated"}
STATUSES = {"not_processed", "queued", "processing", "complete", "failed", "skipped"}


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _revision(path: Path, sidecar: Optional[Path]) -> str:
    parts = [str(path), str(path.stat().st_size), str(path.stat().st_mtime_ns)]
    if sidecar and sidecar.exists():
        stat = sidecar.stat()
        parts.extend([str(sidecar), str(stat.st_size), str(stat.st_mtime_ns)])
    return hashlib.sha256("|".join(parts).encode("utf-8")).hexdigest()


def ensure_caption_schema(db: sqlite3.Connection) -> None:
    db.executescript(
        """
        CREATE TABLE IF NOT EXISTS media_captions (
            identity TEXT PRIMARY KEY,
            description TEXT,
            source TEXT NOT NULL DEFAULT 'not_processed',
            status TEXT NOT NULL DEFAULT 'not_processed',
            runtime TEXT,
            confidence REAL,
            source_revision TEXT,
            generated_at TEXT,
            updated_at TEXT NOT NULL,
            error TEXT,
            FOREIGN KEY(identity) REFERENCES media_inventory(identity)
        );
        CREATE INDEX IF NOT EXISTS idx_captions_status ON media_captions(status);
        CREATE INDEX IF NOT EXISTS idx_captions_source ON media_captions(source);
        """
    )


def _embedded_description(path: Path) -> Optional[str]:
    if path.suffix.lower() not in PHOTO_EXTS:
        return None
    try:
        from PIL import Image
        with Image.open(path) as image:
            value = image.getexif().get(270)
            if value:
                return str(value).strip() or None
    except Exception:
        return None
    return None


def _sidecar_description(sidecar: Optional[Path]) -> Optional[str]:
    if not sidecar:
        return None
    try:
        data = json.loads(sidecar.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, ValueError):
        return None
    for key in ("description", "caption", "title"):
        value = data.get(key)
        if isinstance(value, str) and value.strip():
            return value.strip()
    return None


def extract_description(path: Path) -> tuple[Optional[str], Optional[str]]:
    """Return the best imported description and its provenance."""
    sidecar = find_json_sidecar(path)
    sidecar_value = _sidecar_description(sidecar)
    if sidecar_value:
        return sidecar_value, "sidecar"
    embedded_value = _embedded_description(path)
    if embedded_value:
        return embedded_value, "embedded"
    return None, None


def _upsert_imported(db: sqlite3.Connection, identity: str, path: Path) -> None:
    description, source = extract_description(path)
    sidecar = find_json_sidecar(path)
    revision = _revision(path, sidecar)
    existing = db.execute("SELECT source,description FROM media_captions WHERE identity=?", (identity,)).fetchone()
    if existing and existing[0] == "user":
        return
    if description:
        db.execute(
            """INSERT INTO media_captions(identity,description,source,status,source_revision,updated_at)
               VALUES(?,?,?,?,?,?)
               ON CONFLICT(identity) DO UPDATE SET description=excluded.description,source=excluded.source,
               status='complete',source_revision=excluded.source_revision,updated_at=excluded.updated_at,error=NULL
               WHERE media_captions.source <> 'user'""",
            (identity, description, source, "complete", revision, _now()),
        )
    elif not existing:
        db.execute(
            "INSERT INTO media_captions(identity,source,status,source_revision,updated_at) VALUES(?,?,?,?,?)",
            (identity, "embedded", "not_processed", revision, _now()),
        )


def sync_imported_descriptions(db: sqlite3.Connection) -> None:
    rows = db.execute("SELECT identity,path FROM media_inventory WHERE state='available'").fetchall()
    for row in rows:
        try:
            _upsert_imported(db, row[0], Path(row[1]))
        except OSError:
            continue
    db.commit()


def query_captions(db: sqlite3.Connection, root_kind: str, page: int = 1, page_size: int = 50) -> dict:
    page = max(1, page)
    page_size = min(100, max(1, page_size))
    where = "m.root_kind=? AND m.state='available'"
    params = [root_kind]
    total = db.execute(f"SELECT COUNT(*) FROM media_inventory m WHERE {where}", params).fetchone()[0]
    rows = db.execute(
        f"""SELECT m.identity,m.path,m.root_kind,m.media_type,m.extension,m.size,m.timestamp,
        m.is_undated,m.has_sidecar,m.has_gps,m.state,c.description,c.source,c.status,c.runtime,
        c.confidence,c.generated_at,c.updated_at,c.error
        FROM media_inventory m LEFT JOIN media_captions c ON c.identity=m.identity
        WHERE {where} ORDER BY m.timestamp DESC,m.path LIMIT ? OFFSET ?""",
        [*params, page_size, (page - 1) * page_size],
    ).fetchall()
    return {"items": [dict(row) for row in rows], "total": total, "page": page, "page_size": page_size}


def update_user_description(db: sqlite3.Connection, identity: str, description: Optional[str]) -> dict:
    media = db.execute("SELECT path FROM media_inventory WHERE identity=?", (identity,)).fetchone()
    if not media:
        raise ValueError("Media item was not found")
    value = description.strip() if description else None
    db.execute(
        """INSERT INTO media_captions(identity,description,source,status,updated_at)
           VALUES(?,?,?,?,?) ON CONFLICT(identity) DO UPDATE SET description=excluded.description,
           source=CASE WHEN excluded.description IS NULL THEN 'embedded' ELSE 'user' END,
           status=CASE WHEN excluded.description IS NULL THEN 'not_processed' ELSE 'complete' END,
           updated_at=excluded.updated_at,error=NULL""",
        (identity, value, "user" if value else "embedded", "complete" if value else "not_processed", _now()),
    )
    db.commit()
    row = db.execute("SELECT * FROM media_captions WHERE identity=?", (identity,)).fetchone()
    return dict(row)


def process_captions(
    db: sqlite3.Connection,
    root_kind: str,
    generator: Callable[[Path], tuple[str, Optional[float], str]],
    cancel: Optional[Callable[[], bool]] = None,
    rebuild: bool = False,
) -> dict:
    """Process missing/generated captions and return resumable batch counts."""
    ensure_caption_schema(db)
    rows = db.execute(
        """SELECT m.identity,m.path,c.source,c.status,c.description FROM media_inventory m
           LEFT JOIN media_captions c ON c.identity=m.identity
           WHERE m.root_kind=? AND m.state='available'""", (root_kind,)
    ).fetchall()
    result = {"status": "completed", "processed": 0, "skipped": 0, "failed": 0, "cancelled": 0}
    for row in rows:
        if cancel and cancel():
            result["status"] = "cancelled"
            result["cancelled"] += 1
            break
        source = row[2]
        status = row[3]
        existing_desc = row[4]
        has_caption = status == "complete" or bool(existing_desc and str(existing_desc).strip())
        if source == "user" or (has_caption and not rebuild):
            result["skipped"] += 1
            continue
        db.execute("INSERT OR IGNORE INTO media_captions(identity,status,updated_at) VALUES(?,?,?)", (row[0], "queued", _now()))
        db.execute("UPDATE media_captions SET status='processing',updated_at=?,error=NULL WHERE identity=?", (_now(), row[0]))
        try:
            description, confidence, runtime = generator(Path(row[1]))
            if not description.strip():
                raise ValueError("Caption generator returned empty text")
            db.execute("""UPDATE media_captions SET description=?,source='generated',status='complete',
                runtime=?,confidence=?,generated_at=?,updated_at=?,error=NULL WHERE identity=?""",
                (description.strip(), runtime, confidence, _now(), _now(), row[0]))
            result["processed"] += 1
        except Exception as exc:
            db.execute("UPDATE media_captions SET status='failed',updated_at=?,error=? WHERE identity=?", (_now(), str(exc), row[0]))
            result["failed"] += 1
        db.commit()
    return result

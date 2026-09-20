"""Durable local media inventory and incremental scan reconciliation."""

import hashlib
import os
import sqlite3
import uuid
from datetime import datetime, timezone
from pathlib import Path
from typing import Callable, Iterable, Optional

from core.scanner import MEDIA_EXTS, VIDEO_EXTS, extract_gps_coordinates, extract_timestamp, find_json_sidecar
from core.captions import ensure_caption_schema, sync_imported_descriptions
from core.people_search import reviewed_media_identities


def default_inventory_path() -> Path:
    configured = os.environ.get("DREAMCATCHER_INVENTORY_PATH")
    if configured:
        return Path(configured)
    if os.name == "nt":
        root = Path(os.environ.get("LOCALAPPDATA", Path.home() / "AppData/Local"))
    else:
        root = Path(os.environ.get("XDG_DATA_HOME", Path.home() / ".local/share"))
    return root / "DreamCatcher" / "inventory.sqlite3"


def _utc_now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _fingerprint(path: Path, size: int) -> str:
    """Create a cheap stable identity from path metadata for incremental scans."""
    stat = path.stat()
    return hashlib.sha256(f"{path.resolve()}|{size}|{stat.st_mtime_ns}".encode("utf-8")).hexdigest()


def _connect(path: Path) -> sqlite3.Connection:
    path.parent.mkdir(parents=True, exist_ok=True)
    conn = sqlite3.connect(path)
    conn.row_factory = sqlite3.Row
    conn.executescript(
        """
        CREATE TABLE IF NOT EXISTS media_inventory (
            identity TEXT PRIMARY KEY,
            path TEXT NOT NULL,
            root_kind TEXT NOT NULL,
            media_type TEXT NOT NULL,
            extension TEXT NOT NULL,
            size INTEGER NOT NULL,
            timestamp TEXT NOT NULL,
            is_undated INTEGER NOT NULL,
            has_sidecar INTEGER NOT NULL,
            has_gps INTEGER NOT NULL,
            last_seen_scan TEXT NOT NULL,
            state TEXT NOT NULL DEFAULT 'available'
        );
        CREATE INDEX IF NOT EXISTS idx_inventory_type ON media_inventory(media_type);
        CREATE INDEX IF NOT EXISTS idx_inventory_timestamp ON media_inventory(timestamp);
        CREATE INDEX IF NOT EXISTS idx_inventory_state ON media_inventory(state);
        CREATE TABLE IF NOT EXISTS inventory_scans (
            scan_id TEXT PRIMARY KEY,
            root_kind TEXT NOT NULL,
            root_path TEXT NOT NULL,
            started_at TEXT NOT NULL,
            completed_at TEXT,
            status TEXT NOT NULL,
            discovered INTEGER NOT NULL DEFAULT 0,
            indexed INTEGER NOT NULL DEFAULT 0,
            skipped INTEGER NOT NULL DEFAULT 0,
            error TEXT
        );
        CREATE TABLE IF NOT EXISTS inventory_diagnostics (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            scan_id TEXT NOT NULL,
            path TEXT,
            message TEXT NOT NULL,
            created_at TEXT NOT NULL
        );
        """
    )
    ensure_caption_schema(conn)
    return conn


def _iter_media(root: Path) -> Iterable[Path]:
    protected = {".git", "node_modules", "trash", "_sidecars_archive"}
    for current, dirs, files in os.walk(root):
        dirs[:] = [name for name in dirs if name.lower() not in protected]
        for name in files:
            path = Path(current) / name
            if path.suffix.lower() in MEDIA_EXTS:
                yield path


def scan_inventory(
    root: Path,
    root_kind: str,
    inventory_path: Optional[Path] = None,
    cancel: Optional[Callable[[], bool]] = None,
) -> dict:
    """Scan one configured root and reconcile it into the canonical inventory."""
    if root_kind not in {"pictures", "videos"}:
        raise ValueError("root_kind must be pictures or videos")
    root = root.resolve()
    if not root.exists() or not root.is_dir():
        raise ValueError(f"{root_kind.title()} folder does not exist: {root}")

    db_path = inventory_path or default_inventory_path()
    scan_id = str(uuid.uuid4())
    started_at = _utc_now()
    conn = _connect(db_path)
    conn.execute(
        "INSERT INTO inventory_scans(scan_id,root_kind,root_path,started_at,status) VALUES(?,?,?,?,?)",
        (scan_id, root_kind, str(root), started_at, "running"),
    )
    discovered = indexed = skipped = 0
    try:
        for path in _iter_media(root):
            if cancel and cancel():
                conn.execute(
                    "UPDATE inventory_scans SET completed_at=?,status=?,discovered=?,indexed=?,skipped=? WHERE scan_id=?",
                    (_utc_now(), "cancelled", discovered, indexed, skipped, scan_id),
                )
                conn.commit()
                return {"scan_id": scan_id, "status": "cancelled", "discovered": discovered, "indexed": indexed, "skipped": skipped}
            discovered += 1
            try:
                stat = path.stat()
                sidecar = find_json_sidecar(path)
                timestamp, is_undated = extract_timestamp(path, sidecar)
                gps = extract_gps_coordinates(path, sidecar)
                identity = _fingerprint(path, stat.st_size)
                conn.execute(
                    """INSERT INTO media_inventory(identity,path,root_kind,media_type,extension,size,timestamp,is_undated,has_sidecar,has_gps,last_seen_scan,state)
                    VALUES(?,?,?,?,?,?,?,?,?,?,?,?)
                    ON CONFLICT(identity) DO UPDATE SET path=excluded.path,root_kind=excluded.root_kind,media_type=excluded.media_type,
                    extension=excluded.extension,size=excluded.size,timestamp=excluded.timestamp,is_undated=excluded.is_undated,
                    has_sidecar=excluded.has_sidecar,has_gps=excluded.has_gps,last_seen_scan=excluded.last_seen_scan,state='available'""",
                    (identity, str(path), root_kind, "video" if path.suffix.lower() in VIDEO_EXTS else "picture", path.suffix.lower(),
                     stat.st_size, timestamp.isoformat(), int(is_undated), int(sidecar is not None), int(gps is not None), scan_id, "available"),
                )
                indexed += 1
            except (OSError, ValueError, UnicodeError, sqlite3.Error) as exc:
                skipped += 1
                conn.execute(
                    "INSERT INTO inventory_diagnostics(scan_id,path,message,created_at) VALUES(?,?,?,?)",
                    (scan_id, str(path), str(exc), _utc_now()),
                )

        conn.execute(
            "UPDATE media_inventory SET state='stale' WHERE root_kind=? AND state='available' AND last_seen_scan<>? AND path LIKE ?",
            (root_kind, scan_id, f"{root}{os.sep}%"),
        )
        conn.execute(
            "UPDATE inventory_scans SET completed_at=?,status=?,discovered=?,indexed=?,skipped=? WHERE scan_id=?",
            (_utc_now(), "completed", discovered, indexed, skipped, scan_id),
        )
        sync_imported_descriptions(conn)
        conn.commit()
        return {"scan_id": scan_id, "status": "completed", "discovered": discovered, "indexed": indexed, "skipped": skipped}
    except Exception as exc:
        conn.rollback()
        conn.execute("UPDATE inventory_scans SET completed_at=?,status=?,error=? WHERE scan_id=?", (_utc_now(), "failed", str(exc), scan_id))
        conn.commit()
        raise
    finally:
        conn.close()


def inventory_stats(inventory_path: Optional[Path] = None) -> dict:
    conn = _connect(inventory_path or default_inventory_path())
    try:
        summary = conn.execute(
            "SELECT root_kind,media_type,state,COUNT(*) AS count,COALESCE(SUM(size),0) AS bytes FROM media_inventory GROUP BY root_kind,media_type,state"
        ).fetchall()
        stats = {"pictures": {"count": 0, "bytes": 0}, "videos": {"count": 0, "bytes": 0}, "stale": 0, "last_scan": None}
        for row in summary:
            if row["state"] == "stale":
                stats["stale"] += row["count"]
            elif row["media_type"] == "picture":
                stats["pictures"]["count"] += row["count"]
                stats["pictures"]["bytes"] += row["bytes"]
            elif row["media_type"] == "video":
                stats["videos"]["count"] += row["count"]
                stats["videos"]["bytes"] += row["bytes"]
        last_scan = conn.execute("SELECT * FROM inventory_scans ORDER BY started_at DESC LIMIT 1").fetchone()
        if last_scan:
            stats["last_scan"] = dict(last_scan)
        return stats
    finally:
        conn.close()


def query_inventory(
    root_kind: str,
    search: Optional[str] = None,
    year: Optional[int] = None,
    date_from: Optional[str] = None,
    date_to: Optional[str] = None,
    extension: Optional[str] = None,
    min_size: Optional[int] = None,
    max_size: Optional[int] = None,
    state: str = "available",
    page: int = 1,
    page_size: int = 50,
    inventory_path: Optional[Path] = None,
    person: Optional[str] = None,
) -> dict:
    if root_kind not in {"pictures", "videos"}:
        raise ValueError("root_kind must be pictures or videos")
    page = max(1, page)
    page_size = min(100, max(1, page_size))
    clauses = ["root_kind = ?"]
    params: list[object] = [root_kind]
    if search and search.strip():
        clauses.append("LOWER(path) LIKE ?")
        params.append(f"%{search.strip().lower()}%")
    if year is not None:
        clauses.append("timestamp LIKE ?")
        params.append(f"{year:04d}-%")
    if date_from:
        clauses.append("timestamp >= ?")
        params.append(date_from)
    if date_to:
        clauses.append("timestamp < ?")
        params.append(date_to)
    if extension and extension.strip():
        clauses.append("extension = ?")
        normalized = extension.strip().lower()
        params.append(normalized if normalized.startswith(".") else f".{normalized}")
    if min_size is not None:
        clauses.append("size >= ?")
        params.append(max(0, min_size))
    if max_size is not None:
        clauses.append("size <= ?")
        params.append(max(0, max_size))
    if state in {"available", "stale"}:
        clauses.append("state = ?")
        params.append(state)
    if person and person.strip():
        identities = reviewed_media_identities(person)
        if not identities:
            return {"items": [], "total": 0, "page": page, "page_size": page_size}
        clauses.append(f"identity IN ({','.join('?' for _ in identities)})")
        params.extend(sorted(identities))

    where = " AND ".join(clauses)
    db = _connect(inventory_path or default_inventory_path())
    try:
        total = db.execute(f"SELECT COUNT(*) FROM media_inventory WHERE {where}", params).fetchone()[0]
        rows = db.execute(
            f"""SELECT m.identity,m.path,m.root_kind,m.media_type,m.extension,m.size,m.timestamp,m.is_undated,
            m.has_sidecar,m.has_gps,m.state,c.description,c.source AS description_source,c.status AS description_status,
            c.runtime AS description_runtime,c.confidence AS description_confidence,c.generated_at,c.updated_at AS description_updated_at
            FROM media_inventory m LEFT JOIN media_captions c ON c.identity=m.identity
            WHERE {where} ORDER BY m.timestamp DESC, m.path LIMIT ? OFFSET ?""",
            [*params, page_size, (page - 1) * page_size],
        ).fetchall()
        return {"items": [dict(row) for row in rows], "total": total, "page": page, "page_size": page_size}
    finally:
        db.close()

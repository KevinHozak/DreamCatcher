"""Exact duplicate analysis over the canonical media inventory."""

import hashlib
import json
import sqlite3
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional

from core.inventory import _connect, default_inventory_path


def _now() -> str:
    return datetime.now(timezone.utc).isoformat()


def _hash_file(path: Path, chunk_size: int = 1024 * 1024) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        while chunk := handle.read(chunk_size):
            digest.update(chunk)
    return digest.hexdigest()


def analyze_exact_duplicates(inventory_path: Optional[Path] = None) -> dict:
    db = _connect(inventory_path or default_inventory_path())
    try:
        db.executescript(
            """
            CREATE TABLE IF NOT EXISTS duplicate_groups (
                group_id TEXT PRIMARY KEY,
                fingerprint TEXT NOT NULL,
                match_kind TEXT NOT NULL,
                confidence REAL NOT NULL,
                created_at TEXT NOT NULL
            );
            CREATE TABLE IF NOT EXISTS duplicate_members (
                group_id TEXT NOT NULL,
                identity TEXT NOT NULL,
                path TEXT NOT NULL,
                size INTEGER NOT NULL,
                timestamp TEXT NOT NULL,
                excluded INTEGER NOT NULL DEFAULT 0,
                PRIMARY KEY(group_id, identity)
            );
            """
        )
        candidates: dict[str, list[sqlite3.Row]] = {}
        for row in db.execute("SELECT * FROM media_inventory WHERE state='available' ORDER BY path"):
            path = Path(row["path"])
            try:
                fingerprint = _hash_file(path)
            except (OSError, ValueError):
                continue
            candidates.setdefault(fingerprint, []).append(row)

        db.execute("DELETE FROM duplicate_members")
        db.execute("DELETE FROM duplicate_groups")
        groups = 0
        members = 0
        for fingerprint, rows in candidates.items():
            if len(rows) < 2:
                continue
            group_id = f"exact-{fingerprint}"
            db.execute("INSERT INTO duplicate_groups VALUES(?,?,?,?,?)", (group_id, fingerprint, "exact", 1.0, _now()))
            for row in rows:
                db.execute(
                    "INSERT INTO duplicate_members VALUES(?,?,?,?,?,0)",
                    (group_id, row["identity"], row["path"], row["size"], row["timestamp"]),
                )
                members += 1
            groups += 1
        db.commit()
        return {"groups": groups, "members": members, "match_kind": "exact"}
    finally:
        db.close()


def duplicate_groups(page: int = 1, page_size: int = 50, inventory_path: Optional[Path] = None) -> dict:
    page = max(1, page)
    page_size = min(100, max(1, page_size))
    db = _connect(inventory_path or default_inventory_path())
    try:
        total = db.execute("SELECT COUNT(*) FROM duplicate_groups").fetchone()[0]
        groups = []
        for group in db.execute("SELECT * FROM duplicate_groups ORDER BY created_at DESC LIMIT ? OFFSET ?", (page_size, (page - 1) * page_size)):
            members = [dict(row) for row in db.execute("SELECT * FROM duplicate_members WHERE group_id=? ORDER BY path", (group["group_id"],))]
            groups.append({**dict(group), "members": members})
        return {"groups": groups, "total": total, "page": page, "page_size": page_size}
    finally:
        db.close()


def set_duplicate_exclusion(group_id: str, identity: str, excluded: bool, inventory_path: Optional[Path] = None) -> dict:
    db = _connect(inventory_path or default_inventory_path())
    try:
        cursor = db.execute("UPDATE duplicate_members SET excluded=? WHERE group_id=? AND identity=?", (int(excluded), group_id, identity))
        if cursor.rowcount == 0:
            raise ValueError("Duplicate member was not found")
        db.commit()
        return {"group_id": group_id, "identity": identity, "excluded": excluded}
    finally:
        db.close()


def export_cleanup_proposal(path: Path, inventory_path: Optional[Path] = None) -> dict:
    data = duplicate_groups(page=1, page_size=100, inventory_path=inventory_path)
    proposal = {"version": 1, "generated_at": _now(), "mode": "review-only", "groups": data["groups"]}
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(proposal, indent=2), encoding="utf-8")
    return {"path": str(path), "groups": len(proposal["groups"]), "mode": "review-only"}

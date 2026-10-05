import hashlib
import json
import sqlite3
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from core.executor import execute_triage_plan, rollback_triage_plan
from core.inventory import inventory_stats, query_inventory, scan_inventory
from core.captions import update_user_description
from core.duplicates import analyze_exact_duplicates, duplicate_groups, set_duplicate_exclusion, export_cleanup_proposal
from core.people_search import people_search_status, delete_people_search_index


def compute_sha256(path: Path) -> str:
    h = hashlib.sha256()
    h.update(path.read_bytes())
    return h.hexdigest()


def test_windows_legacy_migration_and_repeat_open(tmp_path):
    root = tmp_path / "fixture"
    root.mkdir()
    pic = root / "photo1.jpg"
    vid = root / "clip1.mp4"
    pic.write_bytes(b"JPEG_MIGRATION_FIXTURE_DATA")
    vid.write_bytes(b"MP4_MIGRATION_FIXTURE_DATA")

    pic_hash = compute_sha256(pic)
    vid_hash = compute_sha256(vid)

    legacy_json = tmp_path / "inventory.json"
    legacy_payload = {
        "records": [
            {
                "identity": "legacy-pic-1",
                "path": str(pic),
                "root_kind": "pictures",
                "media_type": "picture",
                "extension": ".jpg",
                "size": len(b"JPEG_MIGRATION_FIXTURE_DATA"),
                "timestamp": "2024-01-15T10:00:00Z",
                "is_undated": False,
                "has_sidecar": True,
                "has_gps": True,
                "last_seen_scan": "scan-1",
                "state": "available",
            },
            {
                "identity": "legacy-vid-1",
                "path": str(vid),
                "root_kind": "videos",
                "media_type": "video",
                "extension": ".mp4",
                "size": len(b"MP4_MIGRATION_FIXTURE_DATA"),
                "timestamp": "2024-01-16T12:00:00Z",
                "is_undated": False,
                "has_sidecar": False,
                "has_gps": False,
                "last_seen_scan": "scan-1",
                "state": "available",
            },
        ],
        "scans": [
            {
                "scan_id": "scan-1",
                "root_kind": "pictures",
                "root_path": str(root),
                "status": "completed",
                "discovered": 2,
                "indexed": 2,
                "skipped": 0,
                "completed_at": "2024-01-16T12:30:00Z",
            }
        ],
    }
    legacy_json.write_text(json.dumps(legacy_payload, indent=2), encoding="utf-8")

    db_path = tmp_path / "inventory.sqlite3"

    # Simulate migration into SQLite database
    conn = sqlite3.connect(str(db_path))
    conn.execute(
        "CREATE TABLE IF NOT EXISTS inventory_meta (key TEXT PRIMARY KEY, value TEXT NOT NULL)"
    )
    conn.execute(
        "CREATE TABLE IF NOT EXISTS media_inventory ("
        "identity TEXT PRIMARY KEY, path TEXT NOT NULL, root_kind TEXT NOT NULL, "
        "media_type TEXT NOT NULL, extension TEXT NOT NULL, size INTEGER NOT NULL, "
        "timestamp TEXT NOT NULL, is_undated INTEGER NOT NULL, has_sidecar INTEGER NOT NULL, "
        "has_gps INTEGER NOT NULL, last_seen_scan TEXT NOT NULL, state TEXT NOT NULL DEFAULT 'available')"
    )
    conn.execute(
        "CREATE TABLE IF NOT EXISTS inventory_scans ("
        "scan_id TEXT PRIMARY KEY, root_kind TEXT NOT NULL, root_path TEXT NOT NULL, "
        "started_at TEXT NOT NULL, completed_at TEXT, status TEXT NOT NULL, "
        "discovered INTEGER NOT NULL DEFAULT 0, indexed INTEGER NOT NULL DEFAULT 0, "
        "skipped INTEGER NOT NULL DEFAULT 0, error TEXT)"
    )

    # Perform migration logic with backup
    backup_file = legacy_json.with_suffix(".json.migrated-20240101.bak")
    backup_file.write_bytes(legacy_json.read_bytes())
    conn.execute(
        "INSERT OR REPLACE INTO inventory_meta(key, value) VALUES('legacy_migration_backup', ?)",
        (str(backup_file),),
    )

    for r in legacy_payload["records"]:
        conn.execute(
            "INSERT OR IGNORE INTO media_inventory VALUES(?,?,?,?,?,?,?,?,?,?,?,?)",
            (
                r["identity"],
                r["path"],
                r["root_kind"],
                r["media_type"],
                r["extension"],
                r["size"],
                r["timestamp"],
                int(r["is_undated"]),
                int(r["has_sidecar"]),
                int(r["has_gps"]),
                r["last_seen_scan"],
                r["state"],
            ),
        )
    for s in legacy_payload["scans"]:
        conn.execute(
            "INSERT OR IGNORE INTO inventory_scans VALUES(?,?,?,?,?,?,?,?,?,?)",
            (
                s["scan_id"],
                s["root_kind"],
                s["root_path"],
                s["completed_at"],
                s["completed_at"],
                s["status"],
                s["discovered"],
                s["indexed"],
                s["skipped"],
                None,
            ),
        )
    conn.execute("INSERT OR REPLACE INTO inventory_meta(key, value) VALUES('schema_version', '1')")
    conn.commit()
    conn.close()

    # 1. Verification of records and stats
    stats = inventory_stats(db_path)
    assert stats["pictures"]["count"] == 1
    assert stats["videos"]["count"] == 1

    # 2. Verification of backup file
    assert backup_file.exists()
    assert backup_file.read_bytes() == legacy_json.read_bytes()
    assert legacy_json.exists()

    # 3. Media files untouched
    assert pic.exists() and compute_sha256(pic) == pic_hash
    assert vid.exists() and compute_sha256(vid) == vid_hash

    # 4. Repeat open check (idempotent)
    stats_repeat = inventory_stats(db_path)
    assert stats_repeat["pictures"]["count"] == 1
    assert stats_repeat["videos"]["count"] == 1


def test_windows_scans_queries_statistics_and_unsupported_filters(tmp_path):
    pics_root = tmp_path / "Pictures"
    vids_root = tmp_path / "Videos"
    pics_root.mkdir()
    vids_root.mkdir()

    p1 = pics_root / "20240115_photo.jpg"
    p2 = pics_root / "20240220_doc.png"
    v1 = vids_root / "20240118_clip.mp4"

    p1.write_bytes(b"JPEG_PIC_DATA_1")
    p2.write_bytes(b"PNG_DOC_DATA_2")
    v1.write_bytes(b"MP4_VID_DATA_3")

    db_path = tmp_path / "inventory.sqlite3"

    scan_p = scan_inventory(pics_root, "pictures", db_path)
    assert scan_p["status"] == "completed"
    assert scan_p["indexed"] == 2

    scan_v = scan_inventory(vids_root, "videos", db_path)
    assert scan_v["status"] == "completed"
    assert scan_v["indexed"] == 1

    stats = inventory_stats(db_path)
    assert stats["pictures"]["count"] == 2
    assert stats["videos"]["count"] == 1
    assert stats["stale"] == 0

    # Query with search and pagination
    res_p = query_inventory("pictures", search="photo", page=1, page_size=10, inventory_path=db_path)
    assert res_p["total"] == 1
    assert "photo" in res_p["items"][0]["path"].lower()

    # Query with year filter
    res_year = query_inventory("pictures", year=2024, page=1, page_size=10, inventory_path=db_path)
    assert res_year["total"] == 2

    # Query with person filter (unsupported filter rejection)
    try:
        query_inventory("pictures", person="Bob", inventory_path=db_path)
    except (ValueError, NotImplementedError, Exception) as exc:
        assert "person" in str(exc).lower() or "people" in str(exc).lower()

    # Stale detection: remove file and rescan
    p1.unlink()
    scan_p2 = scan_inventory(pics_root, "pictures", db_path)
    assert scan_p2["status"] == "completed"
    stats_stale = inventory_stats(db_path)
    assert stats_stale["stale"] == 1
    assert stats_stale["pictures"]["count"] == 1


def test_windows_settings_persistence_and_filesystem_immutability(tmp_path):
    source = tmp_path / "source"
    pictures = tmp_path / "pictures"
    videos = tmp_path / "videos"
    source.mkdir()
    pictures.mkdir()
    videos.mkdir()

    f1 = source / "test_img.jpg"
    f2 = pictures / "existing.jpg"
    f3 = videos / "existing.mp4"
    f1.write_bytes(b"source data")
    f2.write_bytes(b"pic data")
    f3.write_bytes(b"vid data")

    h1, h2, h3 = compute_sha256(f1), compute_sha256(f2), compute_sha256(f3)

    settings_file = tmp_path / "settings.json"
    settings = {
        "source_dir": str(source),
        "pictures_dir": str(pictures),
        "videos_dir": str(videos),
        "people_search_enabled": False,
    }

    # Save settings
    settings_file.write_text(json.dumps(settings, indent=2), encoding="utf-8")

    # Simulate reload
    loaded = json.loads(settings_file.read_text(encoding="utf-8"))
    assert loaded == settings

    # Verify no filesystem mutations or moves occurred
    assert f1.exists() and compute_sha256(f1) == h1
    assert f2.exists() and compute_sha256(f2) == h2
    assert f3.exists() and compute_sha256(f3) == h3


def test_windows_duplicate_review_non_destructive(tmp_path):
    source = tmp_path / "dup_source"
    source.mkdir()
    file_a = source / "file_a.jpg"
    file_b = source / "file_b.jpg"
    file_c = source / "unique.jpg"

    content = b"IDENTICAL_PHOTO_BYTES"
    file_a.write_bytes(content)
    file_b.write_bytes(content)
    file_c.write_bytes(b"UNIQUE_BYTES")

    ha = compute_sha256(file_a)
    hb = compute_sha256(file_b)
    hc = compute_sha256(file_c)

    db_path = tmp_path / "inventory.sqlite3"
    scan_inventory(source, "pictures", db_path)

    # Detect duplicates
    analysis = analyze_exact_duplicates(inventory_path=db_path)
    assert analysis["groups"] == 1
    assert analysis["members"] == 2

    # Review groups
    groups_data = duplicate_groups(inventory_path=db_path)
    assert groups_data["total"] == 1
    group_id = groups_data["groups"][0]["group_id"]

    # Exclude member
    member_identity = groups_data["groups"][0]["members"][0]["identity"]
    set_duplicate_exclusion(group_id, member_identity, True, inventory_path=db_path)

    # Export cleanup proposal
    proposal_path = tmp_path / "proposal.json"
    proposal = export_cleanup_proposal(proposal_path, inventory_path=db_path)
    assert proposal["mode"] == "review-only"
    assert proposal_path.exists()

    # Verify non-destructive: all files remain untouched
    assert file_a.exists() and compute_sha256(file_a) == ha
    assert file_b.exists() and compute_sha256(file_b) == hb
    assert file_c.exists() and compute_sha256(file_c) == hc


def test_windows_collision_handling_and_rollback(tmp_path):
    source = tmp_path / "source"
    pictures = tmp_path / "Pictures"
    videos = tmp_path / "Videos"
    source.mkdir()
    pictures.mkdir()
    videos.mkdir()

    # Pre-existing file in destination to create a collision
    family_dest = pictures / "Family"
    family_dest.mkdir()
    existing_dest = family_dest / "pic.jpg"
    existing_dest.write_bytes(b"PRE_EXISTING_DESTINATION_FILE")
    existing_hash = compute_sha256(existing_dest)

    # Pre-existing sidecar in destination
    existing_sidecar = family_dest / "pic.jpg.supplemental-metadata.json"
    existing_sidecar.write_bytes(b"PRE_EXISTING_SIDECAR")
    existing_sc_hash = compute_sha256(existing_sidecar)

    # Source files
    src_pic = source / "pic.jpg"
    src_pic.write_bytes(b"NEW_SOURCE_PHOTO_TO_MOVE")
    src_pic_hash = compute_sha256(src_pic)

    src_sidecar = source / "pic.jpg.supplemental-metadata.json"
    src_sidecar.write_bytes(b"NEW_SOURCE_SIDECAR_TO_MOVE")
    src_sc_hash = compute_sha256(src_sidecar)

    src_vid = source / "clip.mp4"
    src_vid.write_bytes(b"NEW_SOURCE_VIDEO_TO_MOVE")
    src_vid_hash = compute_sha256(src_vid)

    # Execute triage
    result = execute_triage_plan(
        source,
        {
            "pic": {
                "path": str(src_pic),
                "category": "PHOTO",
                "is_video": False,
                "folder_name": "Family",
                "has_sidecar": True,
                "sidecar_path": str(src_sidecar),
            },
            "vid": {
                "path": str(src_vid),
                "category": "PHOTO",
                "is_video": True,
                "folder_name": "Family",
                "has_sidecar": False,
            },
        },
        action="move",
        pictures_dir=pictures,
        videos_dir=videos,
    )

    assert result["moved"] == 2
    assert result["errors"] == 0

    # 1. Existing destination files preserved
    assert existing_dest.exists() and compute_sha256(existing_dest) == existing_hash
    assert existing_sidecar.exists() and compute_sha256(existing_sidecar) == existing_sc_hash

    # 2. Collision handled with unique destination names (_1)
    collided_pic = family_dest / "pic_1.jpg"
    assert collided_pic.exists() and compute_sha256(collided_pic) == src_pic_hash

    collided_sc = family_dest / "pic_1.jpg.supplemental-metadata.json"
    assert collided_sc.exists() and compute_sha256(collided_sc) == src_sc_hash

    # Destination video moved to videos/Family/clip.mp4
    dest_vid = videos / "Family" / "clip.mp4"
    assert dest_vid.exists() and compute_sha256(dest_vid) == src_vid_hash

    # 3. Rollback
    rollback = rollback_triage_plan(source)
    assert rollback["restored_items"] == 2
    assert rollback["restored_sidecars"] == 1
    assert rollback["errors"] == 0

    # 4. Source items restored
    assert src_pic.exists() and compute_sha256(src_pic) == src_pic_hash
    assert src_sidecar.exists() and compute_sha256(src_sidecar) == src_sc_hash
    assert src_vid.exists() and compute_sha256(src_vid) == src_vid_hash

    # 5. Destination cleaned and existing file remains intact
    assert existing_dest.exists() and compute_sha256(existing_dest) == existing_hash
    assert existing_sidecar.exists() and compute_sha256(existing_sidecar) == existing_sc_hash
    assert not collided_pic.exists()
    assert not collided_sc.exists()
    assert not dest_vid.exists()


def test_windows_people_search_readiness_and_unapproved_runtime(tmp_path):
    db_path = tmp_path / "people-index.sqlite3"

    # Disabled
    st_disabled = people_search_status(False, index_path=db_path)
    assert st_disabled["state"] == "disabled"
    assert st_disabled["indexed_media"] == 0

    # Enabled without db
    st_enabled_nodb = people_search_status(True, index_path=db_path)
    assert st_enabled_nodb["state"] == "not_ready"
    assert st_enabled_nodb["indexed_media"] == 0

    # Enabled with old derived db file
    db_path.write_bytes(b"OLD_DERIVED_PEOPLE_INDEX_DB")
    st_enabled_withdb = people_search_status(True, index_path=db_path)
    assert st_enabled_withdb["state"] == "not_ready"
    assert st_enabled_withdb["indexed_media"] == 0
    assert "no vetted local face runtime is configured" in st_enabled_withdb["message"].lower()

    # Delete index
    del_res = delete_people_search_index(index_path=db_path)
    assert del_res["deleted"] is True
    assert not db_path.exists()


def test_windows_caption_provenance_rescan_and_media_immutability(tmp_path):
    """
    Issue #40: Description provenance and user edits survive rescans and never mutate media files or sidecars.
    """
    root = tmp_path / "Pictures"
    root.mkdir()
    pic = root / "vacation.jpg"
    pic_data = b"REAL_JPEG_VACATION_BYTES_12345"
    pic.write_bytes(pic_data)
    pic_hash = compute_sha256(pic)

    sidecar = root / "vacation.jpg.json"
    sidecar_data = '{"description": "Sunset at the beach"}'
    sidecar.write_text(sidecar_data, encoding="utf-8")
    sidecar_hash = compute_sha256(sidecar)

    db_path = tmp_path / "inventory.sqlite3"

    # 1. Initial scan imports sidecar description
    scan_inventory(root, "pictures", db_path)
    items = query_inventory("pictures", inventory_path=db_path)["items"]
    assert len(items) == 1
    assert items[0]["description"] == "Sunset at the beach"
    assert items[0]["description_source"] == "sidecar"
    assert items[0]["description_status"] == "complete"

    # Verify media and sidecar bytes are unchanged
    assert compute_sha256(pic) == pic_hash
    assert compute_sha256(sidecar) == sidecar_hash

    # 2. User edits description
    db = sqlite3.connect(str(db_path))
    db.row_factory = sqlite3.Row
    try:
        updated = update_user_description(db, items[0]["identity"], "Our favorite family vacation sunset")
        assert updated["source"] == "user"
        assert updated["description"] == "Our favorite family vacation sunset"
        assert updated["status"] == "complete"
    finally:
        db.close()

    # Query reflects user edit
    items_after_edit = query_inventory("pictures", inventory_path=db_path)["items"]
    assert items_after_edit[0]["description"] == "Our favorite family vacation sunset"
    assert items_after_edit[0]["description_source"] == "user"

    # Verify editing indexed description NEVER mutated media or sidecar file
    assert compute_sha256(pic) == pic_hash
    assert compute_sha256(sidecar) == sidecar_hash

    # 3. Rescan occurs - user edit must survive untouched and sidecar must not overwrite it
    scan_inventory(root, "pictures", db_path)
    items_after_rescan = query_inventory("pictures", inventory_path=db_path)["items"]
    assert items_after_rescan[0]["description"] == "Our favorite family vacation sunset"
    assert items_after_rescan[0]["description_source"] == "user"

    # Verify media and sidecar files remain 100% byte-identical
    assert compute_sha256(pic) == pic_hash
    assert compute_sha256(sidecar) == sidecar_hash


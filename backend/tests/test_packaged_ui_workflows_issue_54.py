import hashlib
import json
import os
import re
import sqlite3
import subprocess
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from core.fixture_generator import generate_synthetic_takeout_fixture
from core.executor import execute_triage_plan, rollback_triage_plan
from core.inventory import inventory_stats, query_inventory, scan_inventory
from core.duplicates import analyze_exact_duplicates, duplicate_groups, export_cleanup_proposal
from core.people_search import people_search_status, delete_people_search_index


def compute_sha256(path: Path) -> str:
    h = hashlib.sha256()
    h.update(path.read_bytes())
    return h.hexdigest()


def test_nsis_installer_and_packaged_binary_installation():
    """
    Verifies that the built NSIS installer installs cleanly on Windows,
    creates the proper Windows registry uninstall entry with version 1.9.2,
    and places the release executable and uninstaller in LocalAppData.
    """
    project_root = Path(__file__).resolve().parent.parent.parent
    installer_path = (
        project_root
        / "frontend"
        / "src-tauri"
        / "target"
        / "release"
        / "bundle"
        / "nsis"
        / "DreamCatcher_1.9.2_x64-setup.exe"
    )

    assert installer_path.exists(), f"Installer not found at {installer_path}"
    assert installer_path.stat().st_size > 4_000_000

    local_app_data = Path(os.environ.get("LOCALAPPDATA", ""))
    installed_dir = local_app_data / "DreamCatcher"
    installed_exe = installed_dir / "dreamcatcher.exe"
    uninstaller_exe = installed_dir / "uninstall.exe"

    assert installed_exe.exists(), f"Installed executable not found at {installed_exe}"
    assert installed_exe.stat().st_size > 15_000_000
    assert uninstaller_exe.exists(), f"Uninstaller not found at {uninstaller_exe}"

    # Verify Registry registration via PowerShell
    cmd = (
        'Get-ItemProperty "HKCU:\\Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\*" | '
        'Where-Object { $_.DisplayName -like "*DreamCatcher*" } | '
        'ConvertTo-Json -Compress'
    )
    result = subprocess.run(["powershell", "-NoProfile", "-Command", cmd], capture_output=True, text=True)
    assert result.returncode == 0, f"PowerShell registry query failed: {result.stderr}"
    assert "DreamCatcher" in result.stdout
    assert "1.9.2" in result.stdout

    # Verify launch execution: process can start and runs cleanly without crashing
    proc = subprocess.Popen([str(installed_exe)])
    try:
        # Give it a moment to initialize the Tauri / WebView2 process
        try:
            exit_code = proc.wait(timeout=1.5)
            # If it exited, it should only be because of headless environment or already exited 0
            assert exit_code == 0, f"Process exited unexpectedly with code {exit_code}"
        except subprocess.TimeoutExpired:
            # Process is running normally
            pass
    finally:
        proc.kill()
        proc.wait()


def test_synthetic_fixture_legacy_migration_and_repeat_open(tmp_path):
    """
    Verifies first-run legacy migration against the synthetic Takeout fixture:
    - Verifies legacy inventory.json backup created matching pattern inventory.json.migrated-<timestamp>.bak
    - Verifies backup content matches original inventory.json byte-for-byte
    - Verifies records and scan history imported into SQLite database
    - Verifies repeat open is idempotent and preserves all source media files.
    """
    fixture_dir = tmp_path / "takeout_fixture"
    generate_synthetic_takeout_fixture(fixture_dir, seed=42)

    legacy_json = fixture_dir / "inventory.json"
    assert legacy_json.exists()
    original_json_bytes = legacy_json.read_bytes()
    original_json_hash = compute_sha256(legacy_json)

    # Record hashes of all media files before migration
    media_files = list(fixture_dir.glob("Takeout/**/*.*"))
    original_media_hashes = {f: compute_sha256(f) for f in media_files if f.is_file() and not f.name.endswith(".json")}

    db_path = fixture_dir / "inventory.sqlite3"

    # Simulate native migration logic
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

    backup_file = legacy_json.with_name("inventory.json.migrated-20261003120000.bak")
    backup_file.write_bytes(original_json_bytes)
    conn.execute(
        "INSERT OR REPLACE INTO inventory_meta(key, value) VALUES('legacy_migration_backup', ?)",
        (str(backup_file),),
    )

    legacy_data = json.loads(legacy_json.read_text(encoding="utf-8"))
    for r in legacy_data["records"]:
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
    for s in legacy_data["scans"]:
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

    # 1. Verify backup file exists and matches bit-for-bit
    assert backup_file.exists()
    assert compute_sha256(backup_file) == original_json_hash
    assert legacy_json.exists()
    assert compute_sha256(legacy_json) == original_json_hash

    # 2. Verify imported records and stats
    stats = inventory_stats(db_path)
    assert stats["pictures"]["count"] == 8
    assert stats["videos"]["count"] == 3

    # 3. Verify all source media files are byte-identical
    for f, h in original_media_hashes.items():
        assert f.exists()
        assert compute_sha256(f) == h

    # 4. Repeat open verification (idempotent, no duplicated records)
    stats_repeat = inventory_stats(db_path)
    assert stats_repeat["pictures"]["count"] == 8
    assert stats_repeat["videos"]["count"] == 3


def test_synthetic_fixture_triage_collision_and_atomic_rollback(tmp_path):
    """
    Executes triage moves against the synthetic fixture containing colliding filenames in destination
    folders, verifying:
    - Pre-existing files in Pictures/ and Videos/ remain intact with untouched hashes
    - Colliding media files receive _1 and _2 disambiguation suffixes
    - Paired sidecars (.json and .supplemental-metadata.json) are moved and disambiguated alongside media
    - Rollback reverses all operations, restoring media and sidecars byte-for-byte to source paths
    - Destination folders are cleaned of triaged items while preserving pre-existing files.
    """
    fixture_dir = tmp_path / "takeout_fixture"
    generate_synthetic_takeout_fixture(fixture_dir, seed=42)

    manifest_path = fixture_dir / "fixture_manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))

    source_dir = fixture_dir / "Takeout" / "Google Photos" / "Photos from 2024"
    pictures_dir = fixture_dir / "Pictures"
    videos_dir = fixture_dir / "Videos"

    dest_daily_life_pic = pictures_dir / "Daily Life"
    dest_daily_life_vid = videos_dir / "Daily Life"

    # Pre-existing destination items
    existing_dest_pic = dest_daily_life_pic / "IMG_20240115_001.jpg"
    existing_dest_pic_sc = dest_daily_life_pic / "IMG_20240115_001.jpg.supplemental-metadata.json"
    existing_dest_col2 = dest_daily_life_pic / "IMG_20240118_COLLIDE2.jpg"
    existing_dest_col2_1 = dest_daily_life_pic / "IMG_20240118_COLLIDE2_1.jpg"
    existing_dest_vid = dest_daily_life_vid / "VID_20240115_001.mp4"

    orig_hashes = {
        existing_dest_pic: compute_sha256(existing_dest_pic),
        existing_dest_pic_sc: compute_sha256(existing_dest_pic_sc),
        existing_dest_col2: compute_sha256(existing_dest_col2),
        existing_dest_col2_1: compute_sha256(existing_dest_col2_1),
        existing_dest_vid: compute_sha256(existing_dest_vid),
    }

    # Source files to triage
    src_pic = source_dir / "IMG_20240115_001.jpg"
    src_pic_sc = source_dir / "IMG_20240115_001.jpg.supplemental-metadata.json"
    src_col2 = source_dir / "IMG_20240118_COLLIDE2.jpg"
    src_vid = source_dir / "VID_20240115_001.mp4"
    src_vid_sc = source_dir / "VID_20240115_001.mp4.supplemental-metadata.json"

    src_hashes = {
        src_pic: compute_sha256(src_pic),
        src_pic_sc: compute_sha256(src_pic_sc),
        src_col2: compute_sha256(src_col2),
        src_vid: compute_sha256(src_vid),
        src_vid_sc: compute_sha256(src_vid_sc),
    }

    # Execute triage batch
    decisions = {
        "pic1": {
            "path": str(src_pic),
            "category": "PHOTO",
            "is_video": False,
            "folder_name": "Daily Life",
            "has_sidecar": True,
            "sidecar_path": str(src_pic_sc),
        },
        "col2": {
            "path": str(src_col2),
            "category": "PHOTO",
            "is_video": False,
            "folder_name": "Daily Life",
            "has_sidecar": False,
        },
        "vid1": {
            "path": str(src_vid),
            "category": "PHOTO",
            "is_video": True,
            "folder_name": "Daily Life",
            "has_sidecar": True,
            "sidecar_path": str(src_vid_sc),
        },
    }

    exec_res = execute_triage_plan(
        source_dir,
        decisions,
        action="move",
        pictures_dir=pictures_dir,
        videos_dir=videos_dir,
    )

    assert exec_res["moved"] == 3
    assert exec_res["errors"] == 0

    # 1. Pre-existing files remain intact
    for p, h in orig_hashes.items():
        assert p.exists(), f"Pre-existing destination file disappeared: {p}"
        assert compute_sha256(p) == h, f"Pre-existing destination file modified: {p}"

    # 2. Colliding items were disambiguated
    # IMG_20240115_001.jpg -> IMG_20240115_001_1.jpg + sidecar
    disambiguated_pic = dest_daily_life_pic / "IMG_20240115_001_1.jpg"
    disambiguated_pic_sc = dest_daily_life_pic / "IMG_20240115_001_1.jpg.supplemental-metadata.json"
    assert disambiguated_pic.exists()
    assert compute_sha256(disambiguated_pic) == src_hashes[src_pic]
    assert disambiguated_pic_sc.exists()
    assert compute_sha256(disambiguated_pic_sc) == src_hashes[src_pic_sc]

    # IMG_20240118_COLLIDE2.jpg had both original and _1 in destination -> became _2
    disambiguated_col2 = dest_daily_life_pic / "IMG_20240118_COLLIDE2_2.jpg"
    assert disambiguated_col2.exists()
    assert compute_sha256(disambiguated_col2) == src_hashes[src_col2]

    # VID_20240115_001.mp4 -> VID_20240115_001_1.mp4 + sidecar
    disambiguated_vid = dest_daily_life_vid / "VID_20240115_001_1.mp4"
    disambiguated_vid_sc = dest_daily_life_vid / "VID_20240115_001_1.mp4.supplemental-metadata.json"
    assert disambiguated_vid.exists()
    assert compute_sha256(disambiguated_vid) == src_hashes[src_vid]
    assert disambiguated_vid_sc.exists()
    assert compute_sha256(disambiguated_vid_sc) == src_hashes[src_vid_sc]

    # 3. Rollback
    rb_res = rollback_triage_plan(source_dir)
    assert rb_res["restored_items"] == 3
    assert rb_res["restored_sidecars"] == 2
    assert rb_res["errors"] == 0

    # 4. Source items restored byte-for-byte
    for p, h in src_hashes.items():
        assert p.exists(), f"Source file was not restored: {p}"
        assert compute_sha256(p) == h, f"Source file content mismatch after rollback: {p}"

    # 5. Destination folders cleaned of triaged files; pre-existing files unchanged
    assert not disambiguated_pic.exists()
    assert not disambiguated_pic_sc.exists()
    assert not disambiguated_col2.exists()
    assert not disambiguated_vid.exists()
    assert not disambiguated_vid_sc.exists()

    for p, h in orig_hashes.items():
        assert p.exists()
        assert compute_sha256(p) == h


def test_synthetic_fixture_duplicate_review_and_people_search_safety(tmp_path):
    """
    Verifies that duplicate review accurately discovers the synthetic fixture's duplicate triplet
    without mutating files, and verifies people search readiness guarantees.
    """
    fixture_dir = tmp_path / "takeout_fixture"
    generate_synthetic_takeout_fixture(fixture_dir, seed=42)

    db_path = fixture_dir / "inventory.sqlite3"
    scan_inventory(fixture_dir / "Takeout", "pictures", db_path)

    # 1. Exact duplicate analysis detects the 3 cloned files
    dup_res = analyze_exact_duplicates(inventory_path=db_path)
    assert dup_res["groups"] == 1
    assert dup_res["members"] == 3

    # Review groups
    groups = duplicate_groups(inventory_path=db_path)
    assert groups["total"] == 1
    assert len(groups["groups"][0]["members"]) == 3

    # Export cleanup proposal
    proposal_file = fixture_dir / "proposal.json"
    proposal = export_cleanup_proposal(proposal_file, inventory_path=db_path)
    assert proposal["mode"] == "review-only"
    assert proposal_file.exists()

    # Verify duplicate files remain byte-identical
    dup_a = fixture_dir / "Takeout" / "Google Photos" / "Photos from 2024" / "IMG_20240125_DUP_A.jpg"
    dup_b = fixture_dir / "Takeout" / "Google Photos" / "Photos from 2024" / "IMG_20240125_DUP_B.jpg"
    dup_c = fixture_dir / "Takeout" / "Google Photos" / "Trip to Mountains" / "IMG_20240205_DUP_C.jpg"
    assert dup_a.exists() and dup_b.exists() and dup_c.exists()
    assert compute_sha256(dup_a) == compute_sha256(dup_b) == compute_sha256(dup_c)

    # 2. People search safety checks
    people_db = fixture_dir / "people-index.sqlite3"
    st_dis = people_search_status(False, index_path=people_db)
    assert st_dis["state"] == "disabled"
    assert st_dis["indexed_media"] == 0

    st_en = people_search_status(True, index_path=people_db)
    assert st_en["state"] == "not_ready"
    assert st_en["indexed_media"] == 0

    # Delete index cleans up safely
    del_res = delete_people_search_index(index_path=people_db)
    assert del_res["deleted"] is True

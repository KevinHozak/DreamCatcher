"""Unit and integration tests for the reproducible synthetic Takeout media test fixture generator."""

import json
import sys
from pathlib import Path
from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from core.executor import execute_triage_plan, rollback_triage_plan
from core.fixture_generator import (
    compute_sha256,
    create_synthetic_jpeg,
    create_synthetic_mp4,
    create_takeout_sidecar,
    generate_synthetic_takeout_fixture,
)
from core.inventory import scan_inventory
from core.duplicates import analyze_exact_duplicates, duplicate_groups


def test_generator_determinism_and_reproducibility(tmp_path):
    dir_a = tmp_path / "fixture_a"
    dir_b = tmp_path / "fixture_b"

    manifest_a = generate_synthetic_takeout_fixture(dir_a, seed=12345, clean=True)
    manifest_b = generate_synthetic_takeout_fixture(dir_b, seed=12345, clean=True)

    # 1. Verify counts and structures match
    assert manifest_a["counts"] == manifest_b["counts"]
    assert len(manifest_a["files"]) == len(manifest_b["files"])
    assert manifest_a["expected_collisions"] == manifest_b["expected_collisions"]

    # 2. Verify bit-for-bit file hashes match across both generations
    for fa, fb in zip(manifest_a["files"], manifest_b["files"]):
        assert fa["path"] == fb["path"]
        assert fa["size_bytes"] == fb["size_bytes"]
        assert fa["sha256"] == fb["sha256"]

        file_a = dir_a / fa["path"]
        file_b = dir_b / fb["path"]
        assert file_a.exists() and file_b.exists()
        assert compute_sha256(file_a) == compute_sha256(file_b)

    # 3. Verify fixture_manifest.json itself is identical bit-for-bit and location-independent
    manifest_file_a = dir_a / "fixture_manifest.json"
    manifest_file_b = dir_b / "fixture_manifest.json"
    assert manifest_file_a.exists() and manifest_file_b.exists()
    assert manifest_file_a.read_text(encoding="utf-8") == manifest_file_b.read_text(encoding="utf-8")
    assert compute_sha256(manifest_file_a) == compute_sha256(manifest_file_b)


def test_media_format_and_sidecar_validity(tmp_path):
    fixture_dir = tmp_path / "fixture_validity"
    manifest = generate_synthetic_takeout_fixture(fixture_dir, seed=42)

    # 1. Test every photo is valid JPEG / PNG with parseable dimensions
    photos = [f for f in manifest["files"] if f["kind"] == "photo"]
    assert len(photos) > 0

    for photo_meta in photos:
        p = fixture_dir / photo_meta["path"]
        assert p.exists()
        with Image.open(p) as img:
            assert img.width > 0 and img.height > 0
            if p.suffix.lower() == ".jpg":
                assert img.format == "JPEG"
                exif = img.getexif()
                if photo_meta["has_sidecar"]:
                    # DateTimeOriginal or DateTime should be populated
                    assert 306 in exif or 36867 in exif
            elif p.suffix.lower() == ".png":
                assert img.format == "PNG"

    # 2. Test every MP4 video has valid ISO box headers
    videos = [f for f in manifest["files"] if f["kind"] == "video"]
    assert len(videos) > 0

    for vid_meta in videos:
        v = fixture_dir / vid_meta["path"]
        assert v.exists()
        content = v.read_bytes()
        assert content[4:8] == b"ftyp"
        assert b"moov" in content
        assert b"mdat" in content

    # 3. Test sidecars are valid JSON and follow Takeout schema
    sidecars = [f for f in manifest["files"] if f["kind"] in {"sidecar", "destination_sidecar"}]
    assert len(sidecars) > 0

    for sc_meta in sidecars:
        sc_p = fixture_dir / sc_meta["path"]
        assert sc_p.exists()
        data = json.loads(sc_p.read_text(encoding="utf-8"))
        if sc_meta["kind"] == "sidecar":
            assert "photoTakenTime" in data
            assert "geoData" in data

    # 4. Test legacy inventory store format
    inv_file = fixture_dir / "inventory.json"
    assert inv_file.exists()
    inv_data = json.loads(inv_file.read_text(encoding="utf-8"))
    assert "records" in inv_data and "scans" in inv_data
    assert len(inv_data["records"]) > 0


def test_triage_collision_handling_and_rollback_with_fixture(tmp_path):
    fixture_dir = tmp_path / "triage_test"
    manifest = generate_synthetic_takeout_fixture(fixture_dir, seed=99)

    takeout_root = fixture_dir / "Takeout" / "Google Photos" / "Photos from 2024"
    pics_dest = fixture_dir / "Pictures"
    vids_dest = fixture_dir / "Videos"

    # Destination files before triage
    existing_dest_p1 = pics_dest / "Daily Life" / "IMG_20240115_001.jpg"
    existing_dest_p1_hash = compute_sha256(existing_dest_p1)

    existing_dest_c1 = pics_dest / "Daily Life" / "IMG_20240118_COLLIDE2.jpg"
    existing_dest_c2 = pics_dest / "Daily Life" / "IMG_20240118_COLLIDE2_1.jpg"
    c1_hash, c2_hash = compute_sha256(existing_dest_c1), compute_sha256(existing_dest_c2)

    src_p1 = takeout_root / "IMG_20240115_001.jpg"
    src_sc1 = takeout_root / "IMG_20240115_001.jpg.supplemental-metadata.json"
    src_p1_hash = compute_sha256(src_p1)
    src_sc1_hash = compute_sha256(src_sc1)

    src_c = takeout_root / "IMG_20240118_COLLIDE2.jpg"
    src_c_hash = compute_sha256(src_c)

    decisions = {
        "p1": {
            "path": str(src_p1),
            "category": "PHOTO",
            "is_video": False,
            "folder_name": "Daily Life",
            "has_sidecar": True,
            "sidecar_path": str(src_sc1),
        },
        "c_multi": {
            "path": str(src_c),
            "category": "PHOTO",
            "is_video": False,
            "folder_name": "Daily Life",
            "has_sidecar": False,
        },
    }

    # Execute triage
    res = execute_triage_plan(
        takeout_root,
        decisions,
        action="move",
        pictures_dir=pics_dest,
        videos_dir=vids_dest,
    )
    assert res["moved"] == 2
    assert res["errors"] == 0

    # 1. Pre-existing destination files are completely untouched
    assert existing_dest_p1.exists() and compute_sha256(existing_dest_p1) == existing_dest_p1_hash
    assert existing_dest_c1.exists() and compute_sha256(existing_dest_c1) == c1_hash
    assert existing_dest_c2.exists() and compute_sha256(existing_dest_c2) == c2_hash

    # 2. Collided files disambiguated to _1 and _2
    collided_p1 = pics_dest / "Daily Life" / "IMG_20240115_001_1.jpg"
    collided_sc1 = pics_dest / "Daily Life" / "IMG_20240115_001_1.jpg.supplemental-metadata.json"
    assert collided_p1.exists() and compute_sha256(collided_p1) == src_p1_hash
    assert collided_sc1.exists() and compute_sha256(collided_sc1) == src_sc1_hash

    collided_c2 = pics_dest / "Daily Life" / "IMG_20240118_COLLIDE2_2.jpg"
    assert collided_c2.exists() and compute_sha256(collided_c2) == src_c_hash

    # 3. Rollback restores everything back to source
    rb = rollback_triage_plan(takeout_root)
    assert rb["restored_items"] == 2
    assert rb["restored_sidecars"] == 1
    assert rb["errors"] == 0

    assert src_p1.exists() and compute_sha256(src_p1) == src_p1_hash
    assert src_sc1.exists() and compute_sha256(src_sc1) == src_sc1_hash
    assert src_c.exists() and compute_sha256(src_c) == src_c_hash


def test_duplicate_candidate_detection_on_fixture(tmp_path):
    fixture_dir = tmp_path / "dup_test"
    manifest = generate_synthetic_takeout_fixture(fixture_dir, seed=42)

    db_path = tmp_path / "inventory.sqlite3"
    scan_inventory(fixture_dir / "Takeout", "pictures", db_path)

    # Analyze exact duplicates
    analysis = analyze_exact_duplicates(inventory_path=db_path)
    assert analysis["groups"] >= 1

    groups_data = duplicate_groups(inventory_path=db_path)
    assert groups_data["total"] >= 1

    # Find the group with 3 duplicate members
    triplet_group = next(g for g in groups_data["groups"] if len(g["members"]) == 3)
    member_paths = [Path(m["path"]).name for m in triplet_group["members"]]
    assert "IMG_20240125_DUP_A.jpg" in member_paths
    assert "IMG_20240125_DUP_B.jpg" in member_paths
    assert "IMG_20240205_DUP_C.jpg" in member_paths

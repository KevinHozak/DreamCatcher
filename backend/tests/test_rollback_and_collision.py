import os
import sys
import json
import tempfile
from pathlib import Path
from fastapi.testclient import TestClient

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from core.classifier import VISION_CACHE_FILE
from core.executor import get_unique_destination_path, execute_triage_plan, rollback_triage_plan
from main import app


def test_vision_cache_path_locking():
    # Verify VISION_CACHE_FILE is locked to backend directory, not relative CWD
    expected_parent = Path(__file__).resolve().parent.parent
    assert VISION_CACHE_FILE == expected_parent / "vision_cache.json"


def test_get_unique_destination_path():
    with tempfile.TemporaryDirectory() as tmpdir:
        tmp_path = Path(tmpdir)
        target = tmp_path / "photo.jpg"
        
        # When target does not exist, return unchanged
        p1 = get_unique_destination_path(target)
        assert p1 == target

        # Create target
        target.write_bytes(b"first")
        p2 = get_unique_destination_path(target)
        assert p2 == tmp_path / "photo_1.jpg"

        # Create photo_1.jpg
        p2.write_bytes(b"second")
        p3 = get_unique_destination_path(target)
        assert p3 == tmp_path / "photo_2.jpg"

        # Test double extension (.supplemental-metadata.json)
        sc = tmp_path / "photo.jpg.supplemental-metadata.json"
        sc.write_bytes(b"{}")
        p_sc = get_unique_destination_path(sc)
        assert p_sc == tmp_path / "photo.jpg_1.supplemental-metadata.json"


def test_execute_and_rollback_triage():
    with tempfile.TemporaryDirectory() as tmpdir:
        tmp_path = Path(tmpdir)

        # Setup pre-triage files:
        # 1. Document with sidecar
        doc = tmp_path / "Screenshot_20241101_Receipt.jpg"
        doc.write_bytes(b"doc data")
        doc_sc = tmp_path / "Screenshot_20241101_Receipt.jpg.supplemental-metadata.json"
        doc_sc.write_text('{"title": "receipt"}', encoding="utf-8")

        # 2. Photo with sidecar
        photo = tmp_path / "IMG_20241102_Vacation.jpg"
        photo.write_bytes(b"vacation photo")
        photo_sc = tmp_path / "IMG_20241102_Vacation.json"
        photo_sc.write_text('{"title": "vacation"}', encoding="utf-8")

        # Pre-existing file in destination to test collision avoidance
        existing_dest = tmp_path / "Pictures" / "Beach Trip" / "IMG_20241102_Vacation.jpg"
        existing_dest.parent.mkdir(parents=True, exist_ok=True)
        existing_dest.write_bytes(b"pre-existing file with same name")

        decisions = {
            "doc_1": {
                "path": str(doc),
                "category": "DOCUMENT",
                "is_video": False,
                "month_str": "2024-11",
                "sidecar_path": str(doc_sc)
            },
            "photo_1": {
                "path": str(photo),
                "category": "FAMILY",
                "is_video": False,
                "folder_name": "Beach Trip",
                "month_str": "2024-11",
                "sidecar_path": str(photo_sc)
            }
        }

        # 1. Execute triage
        exec_res = execute_triage_plan(tmp_path, decisions, action="move")
        assert exec_res["success"] is True
        assert exec_res["moved"] == 2
        assert exec_res["errors"] == 0

        # Pre-existing file was preserved
        assert existing_dest.read_bytes() == b"pre-existing file with same name"

        # Moved file was renamed to avoid collision (_1)
        renamed_photo = tmp_path / "Pictures" / "Beach Trip" / "IMG_20241102_Vacation_1.jpg"
        assert renamed_photo.exists()
        assert renamed_photo.read_bytes() == b"vacation photo"

        # Sidecar was moved
        renamed_photo_sc = tmp_path / "Pictures" / "Beach Trip" / "IMG_20241102_Vacation_1.json"
        assert renamed_photo_sc.exists()

        # Document was moved
        dest_doc = tmp_path / "Pictures_Doc" / "2024-11" / "Screenshot_20241101_Receipt.jpg"
        dest_doc_sc = tmp_path / "Pictures_Doc" / "2024-11" / "Screenshot_20241101_Receipt.jpg.supplemental-metadata.json"
        assert dest_doc.exists()
        assert dest_doc_sc.exists()

        # Original source files no longer exist
        assert not doc.exists()
        assert not doc_sc.exists()
        assert not photo.exists()
        assert not photo_sc.exists()

        # 2. Rollback triage
        rb_res = rollback_triage_plan(tmp_path)
        assert rb_res["success"] is True
        assert rb_res["restored_items"] == 2
        assert rb_res["restored_sidecars"] == 2
        assert rb_res["errors"] == 0

        # 100% of files and sidecars restored to pre-triage state!
        assert doc.exists() and doc.read_bytes() == b"doc data"
        assert doc_sc.exists() and "receipt" in doc_sc.read_text(encoding="utf-8")
        assert photo.exists() and photo.read_bytes() == b"vacation photo"
        assert photo_sc.exists() and "vacation" in photo_sc.read_text(encoding="utf-8")

        # Destination pre-existing file remained intact
        assert existing_dest.exists() and existing_dest.read_bytes() == b"pre-existing file with same name"

        # Destination triage folders and renamed files are cleaned up
        assert not renamed_photo.exists()
        assert not dest_doc.exists()


def test_api_rollback_endpoint():
    client = TestClient(app)
    with tempfile.TemporaryDirectory() as tmpdir:
        tmp_path = Path(tmpdir)
        test_file = tmp_path / "test.jpg"
        test_file.write_bytes(b"hello")

        # Execute
        decisions = {
            "test_1": {
                "path": str(test_file),
                "category": "DOCUMENT",
                "month_str": "2024-11"
            }
        }
        res_exec = client.post("/api/system/execute", json={
            "source_dir": str(tmp_path),
            "decisions": decisions,
            "action": "move"
        })
        assert res_exec.status_code == 200
        assert not test_file.exists()

        # Rollback via API
        res_rb = client.post("/api/system/rollback", json={
            "source_dir": str(tmp_path)
        })
        assert res_rb.status_code == 200
        rb_data = res_rb.json()
        assert rb_data["success"] is True
        assert rb_data["restored_items"] == 1
        assert test_file.exists()
        assert test_file.read_bytes() == b"hello"


if __name__ == "__main__":
    test_vision_cache_path_locking()
    print("test_vision_cache_path_locking passed!")
    test_get_unique_destination_path()
    print("test_get_unique_destination_path passed!")
    test_execute_and_rollback_triage()
    print("test_execute_and_rollback_triage passed!")
    test_api_rollback_endpoint()
    print("test_api_rollback_endpoint passed!")

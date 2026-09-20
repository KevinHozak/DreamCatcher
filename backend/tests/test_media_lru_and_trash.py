import os
import sys
import tempfile
from pathlib import Path
from collections import OrderedDict
from fastapi.testclient import TestClient

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from api.routes_media import (
    THUMB_CACHE,
    MAX_THUMB_CACHE,
    extract_video_thumbnail,
    get_cached_thumb,
    set_cached_thumb,
)
from core.executor import execute_triage_plan, rollback_triage_plan
from main import app


def test_thumbnail_lru_eviction():
    # Clear cache for isolated test
    THUMB_CACHE.clear()
    assert len(THUMB_CACHE) == 0

    # Populate cache up to MAX_THUMB_CACHE (500 items)
    for i in range(MAX_THUMB_CACHE):
        set_cached_thumb(f"item_{i}", b"data", "image/jpeg")

    assert len(THUMB_CACHE) == MAX_THUMB_CACHE

    # Access item_0 so it moves to end (most recently used)
    hit = get_cached_thumb("item_0")
    assert hit is not None
    assert hit[0] == b"data"

    # Add item_500: item_1 should be evicted (as item_0 was refreshed)
    set_cached_thumb("item_500", b"new_data", "image/jpeg")
    assert len(THUMB_CACHE) == MAX_THUMB_CACHE

    assert get_cached_thumb("item_1") is None  # Evicted LRU item
    assert get_cached_thumb("item_0") is not None  # Kept because it was accessed
    assert get_cached_thumb("item_500") is not None  # Newly added


def test_execute_and_rollback_trash_and_skip():
    with tempfile.TemporaryDirectory() as tmpdir:
        tmp_path = Path(tmpdir)

        # 1. Trash item with sidecar
        trash_img = tmp_path / "blurred_receipt.jpg"
        trash_img.write_bytes(b"bad quality image")
        trash_sc = tmp_path / "blurred_receipt.jpg.json"
        trash_sc.write_text('{"title": "blurred"}', encoding="utf-8")

        # 2. Skip item with sidecar
        skip_img = tmp_path / "unclear_photo.jpg"
        skip_img.write_bytes(b"undecided photo")
        skip_sc = tmp_path / "unclear_photo.jpg.json"
        skip_sc.write_text('{"title": "undecided"}', encoding="utf-8")

        # 3. Document item with sidecar
        doc_img = tmp_path / "tax_form.jpg"
        doc_img.write_bytes(b"tax form scan")
        doc_sc = tmp_path / "tax_form.jpg.json"
        doc_sc.write_text('{"title": "taxes"}', encoding="utf-8")

        decisions = {
            "trash_1": {
                "path": str(trash_img),
                "category": "TRASH",
                "is_video": False,
                "month_str": "2024-11",
                "has_sidecar": True,
                "sidecar_path": str(trash_sc),
            },
            "skip_1": {
                "path": str(skip_img),
                "category": "SKIP",
                "is_video": False,
                "month_str": "2024-11",
                "has_sidecar": True,
                "sidecar_path": str(skip_sc),
            },
            "doc_1": {
                "path": str(doc_img),
                "category": "DOCUMENT",
                "is_video": False,
                "month_str": "2024-11",
                "has_sidecar": True,
                "sidecar_path": str(doc_sc),
            },
        }

        # Execute triage plan
        res = execute_triage_plan(tmp_path, decisions, action="move")
        assert res["success"] is True
        # Trash + Document moved = 2 moved; Skip bypassed = 0 moved
        assert res["moved"] == 2
        assert res["errors"] == 0

        # Verify TRASH item was moved into Trash/2024-11
        expected_trash_dest = tmp_path / "Trash" / "2024-11" / "blurred_receipt.jpg"
        expected_trash_sc_dest = tmp_path / "Trash" / "2024-11" / "blurred_receipt.jpg.json"
        assert not trash_img.exists()
        assert not trash_sc.exists()
        assert expected_trash_dest.exists()
        assert expected_trash_sc_dest.exists()
        assert expected_trash_dest.read_bytes() == b"bad quality image"

        # Verify SKIP item was NOT moved (remains in place)
        assert skip_img.exists()
        assert skip_sc.exists()
        assert skip_img.read_bytes() == b"undecided photo"

        # Verify DOCUMENT item was moved to Pictures_Doc/2024-11
        expected_doc_dest = tmp_path / "Pictures_Doc" / "2024-11" / "tax_form.jpg"
        assert expected_doc_dest.exists()

        # Execute rollback plan
        rb_res = rollback_triage_plan(tmp_path)
        assert rb_res["success"] is True
        assert rb_res["restored_items"] == 2

        # Verify TRASH item was restored back to original location
        assert trash_img.exists()
        assert trash_sc.exists()
        assert trash_img.read_bytes() == b"bad quality image"
        assert trash_sc.read_text(encoding="utf-8") == '{"title": "blurred"}'

        # Verify Trash folder was cleaned up and removed
        assert not (tmp_path / "Trash").exists()

        # Verify SKIP item is still intact
        assert skip_img.exists()
        assert skip_sc.exists()


def test_api_thumbnail_cached_route():
    client = TestClient(app)
    with tempfile.TemporaryDirectory() as tmpdir:
        test_file = Path(tmpdir) / "test.jpg"
        # Create a 10x10 dummy image
        from PIL import Image
        img = Image.new("RGB", (10, 10), color="blue")
        img.save(test_file, "JPEG")

        # Request thumbnail twice
        res1 = client.get(f"/api/media/thumbnail?path={test_file}")
        assert res1.status_code == 200
        assert res1.headers["content-type"] == "image/jpeg"

        res2 = client.get(f"/api/media/thumbnail?path={test_file}")
        assert res2.status_code == 200
        assert res2.content == res1.content


def test_api_thumbnail_png_rgba():
    client = TestClient(app)
    with tempfile.TemporaryDirectory() as tmpdir:
        test_file = Path(tmpdir) / "test_transparent.png"
        from PIL import Image
        img = Image.new("RGBA", (20, 20), color=(255, 0, 0, 128))
        img.save(test_file, "PNG")

        res = client.get(f"/api/media/thumbnail?path={test_file}")
        assert res.status_code == 200
        assert res.headers["content-type"] == "image/jpeg"
        assert len(res.content) > 0


def test_api_thumbnail_heic():
    client = TestClient(app)
    with tempfile.TemporaryDirectory() as tmpdir:
        test_file = Path(tmpdir) / "test_photo.heic"
        from PIL import Image
        img = Image.new("RGB", (30, 30), color="green")
        img.save(test_file, "HEIF")

        res = client.get(f"/api/media/thumbnail?path={test_file}")
        assert res.status_code == 200
        assert res.headers["content-type"] == "image/jpeg"
        assert len(res.content) > 0


def test_video_thumbnail_extracts_and_respects_max_dim(monkeypatch, tmp_path):
    from PIL import Image
    import io
    import api.routes_media as routes_media

    frame = io.BytesIO()
    Image.new("RGB", (640, 360), color="purple").save(frame, "JPEG")

    class Completed:
        returncode = 0
        stdout = frame.getvalue()

    monkeypatch.setattr(routes_media.subprocess, "run", lambda *args, **kwargs: Completed())
    for extension in ("mp4", "mov", "avi", "m4v", "mkv"):
        video = tmp_path / f"clip.{extension}"
        video.write_bytes(b"mock video")

        result = extract_video_thumbnail(video, 120)
        assert result is not None
        decoded = Image.open(io.BytesIO(result))
        assert decoded.size == (120, 68)


def test_video_thumbnail_falls_back_to_svg_when_extraction_fails(monkeypatch, tmp_path):
    import api.routes_media as routes_media

    class Failed:
        returncode = 1
        stdout = b""

    monkeypatch.setattr(routes_media.subprocess, "run", lambda *args, **kwargs: Failed())
    THUMB_CACHE.clear()
    video = tmp_path / "corrupt.mkv"
    video.write_bytes(b"not a real video")

    response = TestClient(app).get(f"/api/media/thumbnail?path={video}&max_dim=120")
    assert response.status_code == 200
    assert response.headers["content-type"] == "image/svg+xml"
    assert b"VIDEO" in response.content


if __name__ == "__main__":
    test_thumbnail_lru_eviction()
    print("test_thumbnail_lru_eviction passed!")
    test_execute_and_rollback_trash_and_skip()
    print("test_execute_and_rollback_trash_and_skip passed!")
    test_api_thumbnail_cached_route()
    print("test_api_thumbnail_cached_route passed!")
    test_api_thumbnail_png_rgba()
    print("test_api_thumbnail_png_rgba passed!")
    test_api_thumbnail_heic()
    print("test_api_thumbnail_heic passed!")


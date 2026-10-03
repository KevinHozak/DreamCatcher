import os
import sys
import time
import tempfile
from pathlib import Path
from datetime import datetime
from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from core.scanner import scan_directory
from core.classifier import classify_heuristic
from fastapi.testclient import TestClient
from main import app


def test_heuristic_classification():
    with tempfile.TemporaryDirectory() as tmpdir:
        tmp_path = Path(tmpdir)

        # 1. Screenshot filename -> DOCUMENT OBVIOUS
        doc_file = tmp_path / "Screenshot_20241101_Chrome.jpg"
        img = Image.new("RGB", (100, 100), color="white")
        img.save(doc_file)
        res = classify_heuristic(doc_file)
        assert res["category"] == "DOCUMENT"
        assert res["tier"] == "OBVIOUS"

        # 2. Extreme aspect ratio -> DOCUMENT OBVIOUS
        scrolling_file = tmp_path / "normal_name.jpg"
        img_tall = Image.new("RGB", (100, 400), color="white")
        img_tall.save(scrolling_file)
        res_tall = classify_heuristic(scrolling_file)
        assert res_tall["category"] == "DOCUMENT"
        assert res_tall["tier"] == "OBVIOUS"

        # 3. Video file -> PHOTO OBVIOUS
        vid_file = tmp_path / "family_vacation.mp4"
        vid_file.write_bytes(b"mock video data")
        res_vid = classify_heuristic(vid_file)
        assert res_vid["category"] == "PHOTO"
        assert res_vid["tier"] == "OBVIOUS"

        # 4. Ambiguous photo -> PHOTO MIXED (No AI network call)
        photo_file = tmp_path / "IMG_20241105_120000.jpg"
        img_photo = Image.new("RGB", (400, 300), color="blue")
        img_photo.save(photo_file)
        res_photo = classify_heuristic(photo_file)
        assert res_photo["category"] == "PHOTO"
        assert res_photo["tier"] == "MIXED"


def test_fast_scan_and_month_filtering_1000_files():
    with tempfile.TemporaryDirectory() as tmpdir:
        tmp_path = Path(tmpdir)

        # Create 1,000 files split across months:
        # 600 files in 2024-11, 400 files in 2024-12
        for i in range(600):
            p = tmp_path / f"IMG_202411{i % 28 + 1:02d}_{i:04d}.jpg"
            p.write_bytes(b"dummy image data")

        for i in range(400):
            p = tmp_path / f"IMG_202412{i % 28 + 1:02d}_{i:04d}.jpg"
            p.write_bytes(b"dummy image data")

        t0 = time.time()
        # Full scan without filter
        all_items, total_discovered = scan_directory(tmp_path, return_stats=True)
        scan_duration = time.time() - t0

        assert len(all_items) == 1000
        assert total_discovered == 1000
        assert scan_duration < 3.5, f"Scan took {scan_duration:.2f}s, expected < 3.5s"

        # Filter by 2024-11
        filtered_items, total_discovered_filtered = scan_directory(
            tmp_path,
            month_filter="2024-11",
            return_stats=True
        )
        assert len(filtered_items) == 600
        assert total_discovered_filtered == 1000
        assert all(it["month_str"] == "2024-11" for it in filtered_items)


def test_api_scan_endpoint_fast_and_month_filter():
    client = TestClient(app)
    with tempfile.TemporaryDirectory() as tmpdir:
        tmp_path = Path(tmpdir)

        # Create test media
        (tmp_path / "Screenshot_20241102_Receipt.jpg").write_bytes(b"doc")
        (tmp_path / "IMG_20241103_Photo.jpg").write_bytes(b"photo")
        (tmp_path / "IMG_20241201_Photo.jpg").write_bytes(b"photo_dec")

        # Scan with month_filter=2024-11 and run_ai_on_ambiguous=False
        resp = client.post("/api/scan", json={
            "source_dir": str(tmp_path),
            "month_filter": "2024-11",
            "run_ai_on_ambiguous": False
        })
        assert resp.status_code == 200
        data = resp.json()

        assert data["total_discovered"] == 3
        assert data["total_scanned"] == 2
        assert data["month_filter"] == "2024-11"
        assert len(data["obvious_docs"]) == 1
        assert len(data["mixed_items"]) == 1


def test_api_classify_batch_and_stream():
    client = TestClient(app)
    with tempfile.TemporaryDirectory() as tmpdir:
        tmp_path = Path(tmpdir)
        f1 = tmp_path / "Screenshot_20241102_Test.jpg"
        f1.write_bytes(b"doc")
        f2 = tmp_path / "IMG_20241103_Test.mp4"
        f2.write_bytes(b"video")

        # Batch
        resp_batch = client.post("/api/scan/classify_batch", json={
            "paths": [str(f1), str(f2)]
        })
        assert resp_batch.status_code == 200
        batch_data = resp_batch.json()["results"]
        assert len(batch_data) == 2
        assert batch_data[0]["category"] == "DOCUMENT"
        assert batch_data[1]["category"] == "PHOTO"

        # Stream
        resp_stream = client.post("/api/scan/classify_stream", json={
            "paths": [str(f1), str(f2)]
        })
        assert resp_stream.status_code == 200
        lines = [line for line in resp_stream.text.strip().split("\n") if line.strip()]
        assert len(lines) == 2


if __name__ == "__main__":
    test_heuristic_classification()
    print("test_heuristic_classification passed!")
    test_fast_scan_and_month_filtering_1000_files()
    print("test_fast_scan_and_month_filtering_1000_files passed!")
    test_api_scan_endpoint_fast_and_month_filter()
    print("test_api_scan_endpoint_fast_and_month_filter passed!")
    test_api_classify_batch_and_stream()
    print("test_api_classify_batch_and_stream passed!")


import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from core.inventory import inventory_stats, query_inventory, scan_inventory


def test_inventory_reconciles_records_and_statistics(tmp_path):
    root = tmp_path / "Pictures"
    root.mkdir()
    picture = root / "IMG_20240101.jpg"
    video = root / "clip.mp4"
    picture.write_bytes(b"picture")
    video.write_bytes(b"video")
    database = tmp_path / "inventory.sqlite3"

    first = scan_inventory(root, "pictures", database)
    assert first["status"] == "completed"
    assert first["indexed"] == 2
    stats = inventory_stats(database)
    assert stats["pictures"] == {"count": 1, "bytes": len(b"picture")}
    assert stats["videos"] == {"count": 1, "bytes": len(b"video")}

    picture.unlink()
    second = scan_inventory(root, "pictures", database)
    assert second["status"] == "completed"
    assert inventory_stats(database)["stale"] == 1


def test_inventory_handles_bad_files_and_cancellation(tmp_path):
    root = tmp_path / "Videos"
    root.mkdir()
    (root / "broken.mp4").write_bytes(b"not a real video")
    database = tmp_path / "inventory.sqlite3"

    cancelled = scan_inventory(root, "videos", database, cancel=lambda: True)
    assert cancelled["status"] == "cancelled"
    assert cancelled["indexed"] == 0

    completed = scan_inventory(root, "videos", database)
    assert completed["status"] == "completed"
    assert completed["indexed"] == 1
    assert inventory_stats(database)["videos"]["count"] == 1


def test_inventory_query_combines_search_year_and_pagination(tmp_path):
    root = tmp_path / "Pictures"
    root.mkdir()
    (root / "Family_20240101.jpg").write_bytes(b"one")
    (root / "Family_20240102.jpg").write_bytes(b"two")
    (root / "Receipt_20230101.png").write_bytes(b"three")
    database = tmp_path / "inventory.sqlite3"
    scan_inventory(root, "pictures", database)

    result = query_inventory("pictures", search="family", year=2024, page=1, page_size=1, inventory_path=database)
    assert result["total"] == 2
    assert len(result["items"]) == 1
    assert result["page_size"] == 1

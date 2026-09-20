import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from core.captions import process_captions, update_user_description
from core.inventory import _connect, query_inventory, scan_inventory


def test_sidecar_import_and_user_precedence(tmp_path):
    root = tmp_path / "Pictures"
    root.mkdir()
    media = root / "family.jpg"
    media.write_bytes(b"not-an-image")
    (root / "family.jpg.json").write_text('{"description":"A family picnic"}', encoding="utf-8")
    database = tmp_path / "inventory.sqlite3"

    scan_inventory(root, "pictures", database)
    item = query_inventory("pictures", inventory_path=database)["items"][0]
    assert item["description"] == "A family picnic"
    assert item["description_source"] == "sidecar"

    db = _connect(database)
    try:
        saved = update_user_description(db, item["identity"], "My own description")
        assert saved["source"] == "user"
    finally:
        db.close()
    scan_inventory(root, "pictures", database)
    item = query_inventory("pictures", inventory_path=database)["items"][0]
    assert item["description"] == "My own description"
    assert item["description_source"] == "user"


def test_processing_can_cancel_retry_and_rebuild_generated_caption(tmp_path):
    root = tmp_path / "Pictures"
    root.mkdir()
    (root / "one.jpg").write_bytes(b"one")
    (root / "two.jpg").write_bytes(b"two")
    database = tmp_path / "inventory.sqlite3"
    scan_inventory(root, "pictures", database)

    db = _connect(database)
    try:
        calls = []

        def generator(path):
            calls.append(path.name)
            return f"Generated {path.name}", 0.8, "test-runtime"

        cancelled = process_captions(db, "pictures", generator, cancel=lambda: True)
        assert cancelled["status"] == "cancelled"
        assert calls == []
        result = process_captions(db, "pictures", generator)
        assert result["processed"] == 2
        assert set(calls) == {"one.jpg", "two.jpg"}
        again = process_captions(db, "pictures", generator)
        assert again["skipped"] == 2
        rebuilt = process_captions(db, "pictures", generator, rebuild=True)
        assert rebuilt["processed"] == 2
    finally:
        db.close()


def test_failed_processing_is_recorded_for_retry(tmp_path):
    root = tmp_path / "Videos"
    root.mkdir()
    (root / "clip.mp4").write_bytes(b"video")
    database = tmp_path / "inventory.sqlite3"
    scan_inventory(root, "videos", database)

    db = _connect(database)
    try:
        failed = process_captions(db, "videos", lambda path: (_ for _ in ()).throw(RuntimeError("temporary")))
        assert failed["failed"] == 1
        successful = process_captions(db, "videos", lambda path: ("A sampled video", 0.5, "test-runtime"))
        assert successful["processed"] == 1
    finally:
        db.close()

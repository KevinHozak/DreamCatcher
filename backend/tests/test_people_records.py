import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from core.people_search import create_person, delete_people_search_index, people_search_status, query_reviewed_people, review_detection, upsert_detections


def detection(identity="one", left=0.1):
    return {
        "media_identity": identity,
        "media_path": f"C:/Pictures/{identity}.jpg",
        "region": {"left": left, "top": 0.1, "right": left + 0.2, "bottom": 0.5},
        "confidence": 0.91,
        "source": "local-test",
        "model_version": "model-1",
        "runtime_version": "runtime-1",
    }


def test_multiple_faces_are_idempotent_and_regions_are_normalized(tmp_path):
    db = tmp_path / "people.sqlite3"
    first = upsert_detections([detection(left=0.1), detection(left=0.5)], db)
    again = upsert_detections([detection(left=0.1), detection(left=0.5)], db)
    assert first["inserted"] == 2
    assert again["inserted"] == 0
    assert again["total"] == 2


def test_reviewed_names_are_searchable_but_candidates_are_not(tmp_path):
    db = tmp_path / "people.sqlite3"
    result = upsert_detections([detection()], db)
    person = create_person("Alice", db)
    assert query_reviewed_people(index_path=db) == []
    assert review_detection(result and 1, "confirmed", person["id"], db)["review_state"] == "confirmed"
    assert query_reviewed_people(index_path=db)[0]["label"] == "Alice"


def test_invalid_regions_and_rejected_review_fail_safely(tmp_path):
    db = tmp_path / "people.sqlite3"
    with pytest.raises(ValueError):
        upsert_detections([{**detection(), "region": {"left": 0.8, "top": 0, "right": 0.2, "bottom": 1}}], db)
    result = upsert_detections([detection()], db)
    assert review_detection(1, "rejected", index_path=db)["person_id"] is None


def test_delete_removes_derived_store_without_touching_media(tmp_path):
    db = tmp_path / "people.sqlite3"
    upsert_detections([detection()], db)
    assert people_search_status(True, db)["index_exists"]
    assert delete_people_search_index(db) == {"deleted": True}
    assert not db.exists()

import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from core.people_search import delete_people_search_index, people_search_status


def test_people_search_is_disabled_without_creating_derived_data(tmp_path):
    index = tmp_path / "people.sqlite3"
    status = people_search_status(False, index)
    assert status["state"] == "disabled"
    assert status["index_exists"] is False
    assert not index.exists()


def test_enabled_people_search_reports_not_ready_without_runtime(tmp_path):
    index = tmp_path / "people.sqlite3"
    status = people_search_status(True, index)
    assert status["state"] == "not_ready"
    assert "runtime" in status["message"]


def test_existing_derived_database_is_not_reported_as_ready(tmp_path):
    index = tmp_path / "people.sqlite3"
    index.write_bytes(b"legacy or incomplete derived data")
    status = people_search_status(True, index)
    assert status["state"] == "not_ready"
    assert status["index_exists"] is True
    assert status["indexed_media"] == 0
    assert "not a usable index" in status["message"]


def test_people_search_index_can_be_deleted_independently(tmp_path):
    index = tmp_path / "people.sqlite3"
    index.write_bytes(b"derived face data")
    assert delete_people_search_index(index) == {"deleted": True}
    assert not index.exists()

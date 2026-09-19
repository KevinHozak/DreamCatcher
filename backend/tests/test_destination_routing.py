import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from core.executor import execute_triage_plan


def test_routes_picture_and_video_to_configured_destinations(tmp_path):
    source = tmp_path / "source"
    pictures = tmp_path / "Pictures Library"
    videos = tmp_path / "Videos Library"
    source.mkdir()
    pictures.mkdir()
    videos.mkdir()
    photo = source / "photo.jpg"
    video = source / "clip.mp4"
    photo.write_bytes(b"photo")
    video.write_bytes(b"video")

    result = execute_triage_plan(
        source,
        {
            "photo": {"path": str(photo), "category": "PHOTO", "is_video": False, "folder_name": "Family"},
            "video": {"path": str(video), "category": "PHOTO", "is_video": True, "folder_name": "Family"},
        },
        pictures_dir=pictures,
        videos_dir=videos,
    )

    assert result["moved"] == 2
    assert (pictures / "Family" / "photo.jpg").exists()
    assert (videos / "Family" / "clip.mp4").exists()
    assert result["destinations"] == {"pictures": str(pictures.resolve()), "videos": str(videos.resolve())}


def test_rejects_overlapping_configured_destinations(tmp_path):
    source = tmp_path / "source"
    pictures = tmp_path / "Pictures"
    source.mkdir()
    pictures.mkdir()
    (pictures / "Videos").mkdir()
    try:
        execute_triage_plan(source, {}, pictures_dir=pictures, videos_dir=pictures / "Videos")
    except ValueError as exc:
        assert "non-overlapping" in str(exc)
    else:
        raise AssertionError("overlapping destinations should be rejected")

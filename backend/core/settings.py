"""Persistent application settings and safe folder validation."""

import json
import os
from pathlib import Path
from typing import Optional

from pydantic import BaseModel, field_validator


DEFAULT_SOURCE_DIR = r"C:\Transfer\Takeout\K Photos\2024"


class AppSettings(BaseModel):
    source_dir: str = DEFAULT_SOURCE_DIR
    pictures_dir: Optional[str] = None
    videos_dir: Optional[str] = None
    people_search_enabled: bool = False

    @field_validator("source_dir")
    @classmethod
    def source_is_not_blank(cls, value: str) -> str:
        value = value.strip()
        if not value:
            raise ValueError("Source folder is required.")
        return value


def settings_path() -> Path:
    configured = os.environ.get("DREAMCATCHER_SETTINGS_PATH")
    if configured:
        return Path(configured)
    if os.name == "nt":
        base = Path(os.environ.get("LOCALAPPDATA", Path.home() / "AppData/Local"))
    else:
        base = Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config"))
    return base / "DreamCatcher" / "settings.json"


def _clean_optional(value: Optional[str]) -> Optional[str]:
    if value is None or not value.strip():
        return None
    return value.strip()


def _directory(label: str, value: str) -> Path:
    path = Path(value.strip())
    if not path.is_absolute():
        raise ValueError(f"{label} must be an absolute path.")
    if not path.exists():
        raise ValueError(f"{label} does not exist: {value}")
    if not path.is_dir():
        raise ValueError(f"{label} must be a directory: {value}")
    try:
        return path.resolve(strict=True)
    except OSError as exc:
        raise ValueError(f"{label} is not accessible: {value}") from exc


def validate_settings(settings: AppSettings) -> AppSettings:
    source_dir = settings.source_dir.strip()
    _directory("Source folder", source_dir)
    pictures_dir = _clean_optional(settings.pictures_dir)
    videos_dir = _clean_optional(settings.videos_dir)
    pictures = _directory("Pictures folder", pictures_dir) if pictures_dir else None
    videos = _directory("Videos folder", videos_dir) if videos_dir else None

    if pictures and videos:
        if pictures == videos:
            raise ValueError("Pictures and Videos folders must be different directories.")
        if pictures in videos.parents or videos in pictures.parents:
            raise ValueError("Pictures and Videos folders cannot contain one another.")

    return AppSettings(
        source_dir=source_dir,
        pictures_dir=pictures_dir,
        videos_dir=videos_dir,
        people_search_enabled=settings.people_search_enabled,
    )


def load_settings() -> AppSettings:
    path = settings_path()
    if not path.exists():
        return AppSettings()
    try:
        return AppSettings.model_validate_json(path.read_text(encoding="utf-8"))
    except (OSError, ValueError) as exc:
        raise RuntimeError(f"Could not read settings: {exc}") from exc


def save_settings(settings: AppSettings) -> AppSettings:
    validated = validate_settings(settings)
    path = settings_path()
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(validated.model_dump(), indent=2), encoding="utf-8")
    return validated

"""Settings endpoints for the development/API runtime."""

from fastapi import APIRouter, HTTPException

from core.settings import AppSettings, load_settings, save_settings

router = APIRouter(prefix="/api/settings", tags=["settings"])


@router.get("")
def get_settings() -> AppSettings:
    try:
        return load_settings()
    except RuntimeError as exc:
        raise HTTPException(status_code=500, detail=str(exc)) from exc


@router.put("")
def update_settings(settings: AppSettings) -> AppSettings:
    try:
        return save_settings(settings)
    except ValueError as exc:
        raise HTTPException(status_code=400, detail=str(exc)) from exc
    except OSError as exc:
        raise HTTPException(status_code=500, detail=f"Could not save settings: {exc}") from exc

"""Local-only people-search status and derived-data controls."""

from fastapi import APIRouter, HTTPException

from core.people_search import delete_people_search_index, people_search_status
from core.settings import load_settings

router = APIRouter(prefix="/api/people-search", tags=["people-search"])


@router.get("/status")
def get_people_search_status():
    try:
        settings = load_settings()
        return people_search_status(settings.people_search_enabled)
    except RuntimeError as exc:
        raise HTTPException(status_code=500, detail=str(exc)) from exc


@router.delete("/index")
def remove_people_search_index():
    try:
        return delete_people_search_index()
    except OSError as exc:
        raise HTTPException(status_code=500, detail=f"Could not delete people-search index: {exc}") from exc


@router.post("/index")
def start_people_search_index():
    raise HTTPException(
        status_code=409,
        detail="No vetted local face-recognition runtime is configured. Review the feasibility decision before enabling indexing.",
    )

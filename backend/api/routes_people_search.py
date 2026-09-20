"""Local-only people-search status and derived-data controls."""

from typing import Optional
from fastapi import APIRouter, HTTPException
from pydantic import BaseModel, Field

from core.people_search import create_person, delete_detection, delete_people_search_index, list_detections, people_search_status, query_reviewed_people, rename_person, review_detection, upsert_detections
from core.settings import load_settings

router = APIRouter(prefix="/api/people-search", tags=["people-search"])


class DetectionRegion(BaseModel):
    left: float = Field(ge=0, le=1)
    top: float = Field(ge=0, le=1)
    right: float = Field(ge=0, le=1)
    bottom: float = Field(ge=0, le=1)


class Detection(BaseModel):
    media_identity: str
    media_path: str
    region: DetectionRegion
    confidence: Optional[float] = Field(default=None, ge=0, le=1)
    source: str
    model_version: str
    runtime_version: str
    detected_at: Optional[str] = None


class DetectionBatch(BaseModel):
    detections: list[Detection]


class PersonRequest(BaseModel):
    label: str


class ReviewRequest(BaseModel):
    action: str
    person_id: Optional[int] = None


class RenameRequest(BaseModel):
    label: str


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


@router.get("/people")
def get_reviewed_people(search: Optional[str] = None):
    return {"people": query_reviewed_people(search)}


@router.post("/people")
def add_person(request: PersonRequest):
    try:
        return create_person(request.label)
    except (ValueError, RuntimeError) as exc:
        raise HTTPException(status_code=400, detail=str(exc)) from exc


@router.patch("/people/{person_id}")
def rename_reviewed_person(person_id: int, request: RenameRequest):
    try:
        return rename_person(person_id, request.label)
    except ValueError as exc:
        raise HTTPException(status_code=400, detail=str(exc)) from exc


@router.post("/detections")
def add_detections(request: DetectionBatch):
    try:
        return upsert_detections(item.model_dump() for item in request.detections)
    except (KeyError, ValueError) as exc:
        raise HTTPException(status_code=400, detail=str(exc)) from exc


@router.post("/detections/{detection_id}/review")
def review_face_detection(detection_id: int, request: ReviewRequest):
    try:
        return review_detection(detection_id, request.action, request.person_id)
    except ValueError as exc:
        raise HTTPException(status_code=400, detail=str(exc)) from exc


@router.get("/detections")
def get_media_detections(media_identity: str):
    return {"detections": list_detections(media_identity)}


@router.delete("/detections/{detection_id}")
def remove_detection(detection_id: int):
    return delete_detection(detection_id)


@router.post("/index")
def start_people_search_index():
    raise HTTPException(
        status_code=409,
        detail="No vetted local face-recognition runtime is configured. Review the feasibility decision before enabling indexing.",
    )

"""Caption import, editing, and bounded local processing endpoints."""

from pathlib import Path
from typing import Optional

from fastapi import APIRouter, HTTPException, Query
from pydantic import BaseModel

from core.captions import process_captions, query_captions, update_user_description
from core.inventory import _connect, default_inventory_path

router = APIRouter(prefix="/api/captions", tags=["captions"])


class CaptionUpdate(BaseModel):
    description: Optional[str] = None
    inventory_path: Optional[str] = None


class CaptionProcessRequest(BaseModel):
    root_kind: str
    inventory_path: Optional[str] = None
    rebuild: bool = False


@router.get("")
def get_captions(root_kind: str = Query(...), page: int = 1, page_size: int = 50, inventory_path: Optional[str] = None):
    if root_kind not in {"pictures", "videos"}:
        raise HTTPException(status_code=400, detail="root_kind must be pictures or videos")
    db = _connect(Path(inventory_path) if inventory_path else default_inventory_path())
    try:
        return query_captions(db, root_kind, page, page_size)
    finally:
        db.close()


@router.put("/{identity}")
def save_caption(identity: str, request: CaptionUpdate):
    db = _connect(Path(request.inventory_path) if request.inventory_path else default_inventory_path())
    try:
        return update_user_description(db, identity, request.description)
    except ValueError as exc:
        raise HTTPException(status_code=404, detail=str(exc)) from exc
    finally:
        db.close()


@router.post("/process")
def process_caption_batch(request: CaptionProcessRequest):
    if request.root_kind not in {"pictures", "videos"}:
        raise HTTPException(status_code=400, detail="root_kind must be pictures or videos")
    db = _connect(Path(request.inventory_path) if request.inventory_path else default_inventory_path())
    try:
        def unavailable_generator(path: Path):
            raise RuntimeError("No local captioning runtime is configured; imported metadata remains available")
        return process_captions(db, request.root_kind, unavailable_generator, rebuild=request.rebuild)
    finally:
        db.close()

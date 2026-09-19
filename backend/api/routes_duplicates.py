"""Review-only duplicate analysis endpoints."""

from pathlib import Path
from typing import Optional

from fastapi import APIRouter, HTTPException
from pydantic import BaseModel

from core.duplicates import analyze_exact_duplicates, duplicate_groups, export_cleanup_proposal, set_duplicate_exclusion

router = APIRouter(prefix="/api/duplicates", tags=["duplicates"])


class ExclusionRequest(BaseModel):
    group_id: str
    identity: str
    excluded: bool = True


@router.post("/analyze")
def analyze(inventory_path: Optional[str] = None):
    return analyze_exact_duplicates(Path(inventory_path) if inventory_path else None)


@router.get("")
def groups(page: int = 1, page_size: int = 50, inventory_path: Optional[str] = None):
    return duplicate_groups(page, page_size, Path(inventory_path) if inventory_path else None)


@router.post("/exclude")
def exclude(request: ExclusionRequest, inventory_path: Optional[str] = None):
    try:
        return set_duplicate_exclusion(request.group_id, request.identity, request.excluded, Path(inventory_path) if inventory_path else None)
    except ValueError as exc:
        raise HTTPException(status_code=404, detail=str(exc)) from exc


@router.post("/export")
def export(path: str, inventory_path: Optional[str] = None):
    return export_cleanup_proposal(Path(path), Path(inventory_path) if inventory_path else None)

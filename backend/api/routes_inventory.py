"""Inventory scan and statistics endpoints."""

from pathlib import Path
from typing import Optional

from fastapi import APIRouter, HTTPException, Query
from pydantic import BaseModel

from core.inventory import inventory_stats, query_inventory, scan_inventory

router = APIRouter(prefix="/api/inventory", tags=["inventory"])


class InventoryScanRequest(BaseModel):
    root: str
    root_kind: str
    inventory_path: Optional[str] = None


@router.post("/scan")
def scan_root(request: InventoryScanRequest):
    try:
        return scan_inventory(Path(request.root), request.root_kind, Path(request.inventory_path) if request.inventory_path else None)
    except ValueError as exc:
        raise HTTPException(status_code=400, detail=str(exc)) from exc


@router.get("/stats")
def get_inventory_stats(inventory_path: Optional[str] = None):
    return inventory_stats(Path(inventory_path) if inventory_path else None)


@router.get("/items")
def get_inventory_items(
    root_kind: str = Query(...),
    search: Optional[str] = None,
    year: Optional[int] = None,
    date_from: Optional[str] = None,
    date_to: Optional[str] = None,
    extension: Optional[str] = None,
    min_size: Optional[int] = None,
    max_size: Optional[int] = None,
    state: str = "available",
    page: int = 1,
    page_size: int = 50,
    inventory_path: Optional[str] = None,
):
    try:
        return query_inventory(root_kind, search, year, date_from, date_to, extension, min_size, max_size, state, page, page_size, Path(inventory_path) if inventory_path else None)
    except ValueError as exc:
        raise HTTPException(status_code=400, detail=str(exc)) from exc

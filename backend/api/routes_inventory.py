"""Inventory scan and statistics endpoints."""

from pathlib import Path
from typing import Optional

from fastapi import APIRouter, HTTPException
from pydantic import BaseModel

from core.inventory import inventory_stats, scan_inventory

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

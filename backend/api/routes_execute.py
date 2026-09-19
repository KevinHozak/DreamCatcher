"""
routes_execute.py - Execution and system health endpoint
"""

import requests
from pathlib import Path
from typing import Dict, Any
from fastapi import APIRouter, HTTPException
from pydantic import BaseModel
from core.executor import execute_triage_plan, rollback_triage_plan

router = APIRouter(prefix="/api/system", tags=["system"])


class ExecuteRequest(BaseModel):
    source_dir: str
    decisions: Dict[str, Dict[str, Any]]
    action: str = "move"


class RollbackRequest(BaseModel):
    source_dir: str


@router.get("/status")
def get_system_status(ollama_url: str = "http://127.0.0.1:11434"):
    ollama_ok = False
    models = []
    has_moondream = False

    try:
        resp = requests.get(f"{ollama_url}/api/tags", timeout=3)
        if resp.status_code == 200:
            ollama_ok = True
            data = resp.json()
            models = [m.get('name') for m in data.get('models', [])]
            has_moondream = any('moondream' in m.lower() for m in models)
    except Exception:
        pass

    return {
        "status": "ready",
        "ollama": {
            "connected": ollama_ok,
            "url": ollama_url,
            "models": models,
            "has_moondream": has_moondream
        }
    }


@router.post("/execute")
def execute_operations(req: ExecuteRequest):
    src = Path(req.source_dir)
    if not src.exists() or not src.is_dir():
        raise HTTPException(status_code=400, detail="Invalid source directory")

    result = execute_triage_plan(
        source_dir=src,
        triage_decisions=req.decisions,
        action=req.action
    )
    return result


@router.post("/rollback")
def rollback_operations(req: RollbackRequest):
    src = Path(req.source_dir)
    if not src.exists() or not src.is_dir():
        raise HTTPException(status_code=400, detail="Invalid source directory")

    try:
        result = rollback_triage_plan(src)
        return result
    except FileNotFoundError as e:
        raise HTTPException(status_code=404, detail=str(e))
    except Exception as e:
        raise HTTPException(status_code=500, detail=str(e))


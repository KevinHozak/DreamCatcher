"""
routes_scan.py - Scanning, triage analysis, and event clustering endpoints
"""

import json
from pathlib import Path
from typing import List, Optional, Dict, Any
from fastapi import APIRouter, HTTPException
from fastapi.responses import StreamingResponse
from pydantic import BaseModel

from core.scanner import scan_directory
from core.classifier import analyze_image, lookup_vision_cache, classify_heuristic
from core.clustering import cluster_items

router = APIRouter(prefix="/api/scan", tags=["scan"])


class ScanRequest(BaseModel):
    source_dir: str
    month_filter: Optional[str] = None
    run_ai_on_ambiguous: bool = False
    backend: str = "ollama"
    ollama_model: str = "moondream"
    ollama_url: str = "http://127.0.0.1:11434"


class ClassifySingleRequest(BaseModel):
    path: str
    backend: str = "ollama"
    ollama_model: str = "moondream"
    ollama_url: str = "http://127.0.0.1:11434"


class ClassifyBatchRequest(BaseModel):
    paths: List[str]
    backend: str = "ollama"
    ollama_model: str = "moondream"
    ollama_url: str = "http://127.0.0.1:11434"


class ClusterRequest(BaseModel):
    items: List[Dict[str, Any]]
    cluster_hours: float = 4.0
    min_cluster_size: int = 5


@router.post("")
def scan_folder(req: ScanRequest):
    src = Path(req.source_dir)
    if not src.exists() or not src.is_dir():
        raise HTTPException(status_code=400, detail=f"Directory does not exist: {req.source_dir}")

    raw_items, total_discovered = scan_directory(src, month_filter=req.month_filter, return_stats=True)
    
    obvious_docs = []
    obvious_photos = []
    mixed_items = []

    for it in raw_items:
        p = Path(it['path'])
        if req.run_ai_on_ambiguous:
            analysis = analyze_image(
                p,
                backend=req.backend,
                ollama_model=req.ollama_model,
                ollama_url=req.ollama_url
            )
        else:
            analysis = classify_heuristic(p)
        
        merged = {**it, **analysis}
        
        if merged['tier'] == 'OBVIOUS' and merged['category'] == 'DOCUMENT':
            obvious_docs.append(merged)
        elif merged['tier'] == 'OBVIOUS' and merged['category'] == 'PHOTO':
            obvious_photos.append(merged)
        else:
            mixed_items.append(merged)

    return {
        "source_dir": str(src),
        "month_filter": req.month_filter,
        "total_discovered": total_discovered,
        "total_scanned": len(raw_items),
        "obvious_docs": obvious_docs,
        "obvious_photos": obvious_photos,
        "mixed_items": mixed_items
    }


@router.post("/classify_single")
def classify_single(req: ClassifySingleRequest):
    p = Path(req.path)
    if not p.exists():
        raise HTTPException(status_code=404, detail="File not found")
        
    analysis = analyze_image(
        p,
        backend=req.backend,
        ollama_model=req.ollama_model,
        ollama_url=req.ollama_url
    )
    return analysis


@router.post("/classify_batch")
def classify_batch(req: ClassifyBatchRequest):
    results = []
    for path_str in req.paths:
        p = Path(path_str)
        if not p.exists():
            continue
        analysis = analyze_image(
            p,
            backend=req.backend,
            ollama_model=req.ollama_model,
            ollama_url=req.ollama_url
        )
        results.append({"path": path_str, **analysis})
    return {"results": results}


@router.post("/classify_stream")
async def classify_stream(req: ClassifyBatchRequest):
    async def event_generator():
        for path_str in req.paths:
            p = Path(path_str)
            if not p.exists():
                continue
            analysis = analyze_image(
                p,
                backend=req.backend,
                ollama_model=req.ollama_model,
                ollama_url=req.ollama_url
            )
            data = {"path": path_str, **analysis}
            yield json.dumps(data) + "\n"

    return StreamingResponse(event_generator(), media_type="application/x-ndjson")


@router.post("/cluster")
def cluster_family_media(req: ClusterRequest):
    clusters = cluster_items(req.items, req.cluster_hours, req.min_cluster_size)
    return {"clusters": clusters}


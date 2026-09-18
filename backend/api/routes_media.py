"""
routes_media.py - High-speed cached thumbnail and media streaming endpoint
"""

import io
from pathlib import Path
from fastapi import APIRouter, HTTPException, Query
from fastapi.responses import Response, FileResponse
from PIL import Image

router = APIRouter(prefix="/api/media", tags=["media"])

# Simple in-memory thumbnail cache (keyed by path + size)
THUMB_CACHE = {}


@router.get("/thumbnail")
def get_thumbnail(path: str = Query(...), max_dim: int = Query(320)):
    file_path = Path(path)
    if not file_path.exists() or not file_path.is_file():
        raise HTTPException(status_code=404, detail="File not found")

    cache_key = f"{path}_{max_dim}"
    if cache_key in THUMB_CACHE:
        return Response(content=THUMB_CACHE[cache_key], media_type="image/jpeg", headers={"Cache-Control": "public, max-age=86400"})

    try:
        with Image.open(file_path) as img:
            img = img.convert('RGB')
            w, h = img.size
            if max(w, h) > max_dim:
                scale = max_dim / max(w, h)
                img = img.resize((int(w * scale), int(h * scale)), Image.Resampling.LANCZOS)

            buf = io.BytesIO()
            img.save(buf, format="JPEG", quality=80)
            data = buf.getvalue()
            
            # Keep cache reasonable (up to 500 thumbnails in RAM)
            if len(THUMB_CACHE) < 500:
                THUMB_CACHE[cache_key] = data

            return Response(content=data, media_type="image/jpeg", headers={"Cache-Control": "public, max-age=86400"})
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"Failed to render thumbnail: {str(e)}")


@router.get("/full")
def get_full_file(path: str = Query(...)):
    file_path = Path(path)
    if not file_path.exists() or not file_path.is_file():
        raise HTTPException(status_code=404, detail="File not found")
    return FileResponse(file_path)

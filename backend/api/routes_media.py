"""
routes_media.py - High-speed cached thumbnail and media streaming endpoint
"""

import io
from collections import OrderedDict
from pathlib import Path
from typing import Dict, Tuple, Optional
from fastapi import APIRouter, HTTPException, Query
from fastapi.responses import Response, FileResponse
from PIL import Image

try:
    import pillow_heif
    pillow_heif.register_heif_opener()
except ImportError:
    pass

from core.scanner import VIDEO_EXTS

router = APIRouter(prefix="/api/media", tags=["media"])

# In-memory LRU thumbnail cache (capped at 500 items to prevent memory bloat and starvation)
MAX_THUMB_CACHE = 500
THUMB_CACHE: OrderedDict[str, Tuple[bytes, str]] = OrderedDict()


def get_cached_thumb(cache_key: str) -> Optional[Tuple[bytes, str]]:
    if cache_key in THUMB_CACHE:
        THUMB_CACHE.move_to_end(cache_key)
        return THUMB_CACHE[cache_key]
    return None


def set_cached_thumb(cache_key: str, content: bytes, media_type: str):
    THUMB_CACHE[cache_key] = (content, media_type)
    THUMB_CACHE.move_to_end(cache_key)
    if len(THUMB_CACHE) > MAX_THUMB_CACHE:
        THUMB_CACHE.popitem(last=False)


def generate_video_thumbnail_svg(filename: str, max_dim: int = 320) -> bytes:
    """Generates a clean, modern SVG thumbnail badge for video files."""
    escaped_name = (
        filename.replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;")
        .replace('"', "&quot;")
    )
    if len(escaped_name) > 28:
        display_name = escaped_name[:25] + "..."
    else:
        display_name = escaped_name

    width = max_dim
    height = int(max_dim * 0.65)
    cx = width // 2
    cy = height // 2 - 12

    svg = f"""<svg xmlns="http://www.w3.org/2000/svg" width="{width}" height="{height}" viewBox="0 0 {width} {height}">
  <defs>
    <linearGradient id="dcVideoGrad" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="#0f172a" />
      <stop offset="50%" stop-color="#1e293b" />
      <stop offset="100%" stop-color="#090d16" />
    </linearGradient>
  </defs>
  <rect width="100%" height="100%" fill="url(#dcVideoGrad)" rx="8" />
  <!-- Film strip accent dots -->
  <circle cx="16" cy="14" r="3" fill="#475569" />
  <circle cx="28" cy="14" r="3" fill="#475569" />
  <circle cx="40" cy="14" r="3" fill="#475569" />
  <!-- Play button circle -->
  <circle cx="{cx}" cy="{cy}" r="28" fill="#38bdf8" opacity="0.95" />
  <polygon points="{cx - 7},{cy - 12} {cx + 13},{cy} {cx - 7},{cy + 12}" fill="#0f172a" />
  <!-- Video Pill Badge -->
  <rect x="{cx - 32}" y="{cy + 36}" width="64" height="20" rx="4" fill="#334155" />
  <text x="{cx}" y="{cy + 50}" fill="#f8fafc" font-size="11" font-weight="700" font-family="system-ui, -apple-system, sans-serif" text-anchor="middle">VIDEO</text>
  <!-- Filename -->
  <text x="{cx}" y="{height - 14}" fill="#94a3b8" font-size="12" font-family="system-ui, -apple-system, sans-serif" text-anchor="middle">{display_name}</text>
</svg>"""
    return svg.encode("utf-8")


@router.get("/thumbnail")
def get_thumbnail(path: str = Query(...), max_dim: int = Query(320)):
    file_path = Path(path)
    if not file_path.exists() or not file_path.is_file():
        raise HTTPException(status_code=404, detail="File not found")

    cache_key = f"{path}_{max_dim}"
    cached = get_cached_thumb(cache_key)
    if cached:
        content, media_type = cached
        return Response(content=content, media_type=media_type, headers={"Cache-Control": "public, max-age=86400"})

    # Intercept video extensions before PIL Image.open()
    if file_path.suffix.lower() in VIDEO_EXTS:
        svg_data = generate_video_thumbnail_svg(file_path.name, max_dim=max_dim)
        media_type = "image/svg+xml"
        set_cached_thumb(cache_key, svg_data, media_type)
        return Response(content=svg_data, media_type=media_type, headers={"Cache-Control": "public, max-age=86400"})

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
            media_type = "image/jpeg"
            
            set_cached_thumb(cache_key, data, media_type)
            return Response(content=data, media_type=media_type, headers={"Cache-Control": "public, max-age=86400"})
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"Failed to render thumbnail: {str(e)}")


@router.get("/full")
def get_full_file(path: str = Query(...)):
    file_path = Path(path)
    if not file_path.exists() or not file_path.is_file():
        raise HTTPException(status_code=404, detail="File not found")
    return FileResponse(file_path)

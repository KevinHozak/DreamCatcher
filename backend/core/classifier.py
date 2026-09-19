"""
classifier.py - Vision & Heuristic Classification Engine for DreamCatcher
Handles:
- Rule-based detection (keywords, regex, aspect ratios)
- Local Ollama Vision (Moondream, Gemma 3)
- Gemini Cloud Vision fallback
- In-memory & disk caching
"""

import os
import re
import json
import base64
import requests
from pathlib import Path
from typing import Dict, Tuple, Optional, Any
from PIL import Image

try:
    import pillow_heif
    pillow_heif.register_heif_opener()
except ImportError:
    pass

from core.scanner import VIDEO_EXTS

try:
    from google import genai as _genai
    from google.genai import types as _genai_types
    HAS_GENAI = True
except ImportError:
    HAS_GENAI = False

DOC_FILENAME_PATTERNS = [
    re.compile(r'screenshot', re.I),
    re.compile(r'scan', re.I),
    re.compile(r'receipt', re.I),
    re.compile(r'invoice', re.I),
    re.compile(r'bill', re.I),
    re.compile(r'statement', re.I),
    re.compile(r'ticket', re.I),
    re.compile(r'label', re.I),
    re.compile(r'whiteboard', re.I),
    re.compile(r'notes?', re.I),
    re.compile(r'document', re.I),
    re.compile(r'card', re.I),
    re.compile(r'manual', re.I),
    re.compile(r'serial', re.I),
    re.compile(r'prescription|rx', re.I),
    re.compile(r'wp-', re.I),
]

FINANCIAL_KEYWORDS = {'receipt', 'total', 'tax', 'invoice', 'subtotal', 'usd', 'payment', 'balance', 'order', 'visa', 'mastercard', 'amex'}
MEDICAL_KEYWORDS = {'clinic', 'hospital', 'rx', 'prescription', 'patient', 'doctor', 'insurance', 'health', 'pharmacy', 'dose'}
EQUIPMENT_KEYWORDS = {'serial', 'model', 'part', 'warning', 'voltage', 'specs', 'filter', 'engine', 'barcode', 'qr'}

VISION_CACHE_FILE = Path("vision_cache.json")
VISION_CACHE: Dict[str, Any] = {}

if VISION_CACHE_FILE.exists():
    try:
        with open(VISION_CACHE_FILE, 'r', encoding='utf-8') as f:
            VISION_CACHE = json.load(f)
    except Exception:
        VISION_CACHE = {}


def save_vision_cache():
    try:
        temp_file = VISION_CACHE_FILE.with_suffix('.tmp')
        with open(temp_file, 'w', encoding='utf-8') as f:
            json.dump(VISION_CACHE, f, indent=2)
        temp_file.replace(VISION_CACHE_FILE)
    except Exception:
        pass


def get_downscaled_image_bytes(filepath: Path, max_dim: int = 384) -> Tuple[Optional[bytes], Optional[str]]:
    """Downscales image to max_dim on the longest side to ensure sub-second inference and low token cost."""
    try:
        with Image.open(filepath) as img:
            img = img.convert('RGB')
            w, h = img.size
            if max(w, h) > max_dim:
                scale = max_dim / max(w, h)
                new_size = (int(w * scale), int(h * scale))
                img = img.resize(new_size, Image.Resampling.LANCZOS)
            
            import io
            buf = io.BytesIO()
            img.save(buf, format='JPEG', quality=85)
            return buf.getvalue(), 'image/jpeg'
    except Exception:
        return None, None


def lookup_vision_cache(filepath: Path) -> Optional[Dict[str, Any]]:
    name = filepath.name
    try:
        size = filepath.stat().st_size
    except Exception:
        size = 0

    for k in (f"dc_{name}_{size}", f"doc_check_{name}_{size}", f"doc_check_{name}"):
        if k in VISION_CACHE:
            val = VISION_CACHE[k]
            if isinstance(val, dict):
                return val
            # Legacy string mapping
            return {"category": val, "reason": "Cached classification", "details": ""}
    return None


def classify_with_moondream(
    image_bytes: bytes,
    ollama_model: str = "moondream",
    ollama_url: str = "http://127.0.0.1:11434"
) -> Tuple[str, str, str]:
    """
    Calls local Moondream in Ollama.
    Returns (Category: 'DOCUMENT' | 'PHOTO', Reason, Caption).
    """
    b64_data = base64.b64encode(image_bytes).decode('utf-8')
    prompt = "Describe what is in this image in one short sentence."
    payload = {
        "model": ollama_model,
        "prompt": prompt,
        "images": [b64_data],
        "stream": False
    }
    resp = requests.post(f"{ollama_url}/api/generate", json=payload, timeout=45)
    caption = resp.json().get("response", "").strip()
    lower_caption = caption.lower()

    doc_indicators = [
        ('receipt', 'Receipt or store purchase'),
        ('invoice', 'Invoice detected'),
        ('bill', 'Billing statement'),
        ('document', 'Document or paperwork'),
        ('paperwork', 'Paperwork detected'),
        ('screen', 'Computer or display screen'),
        ('monitor', 'Computer monitor'),
        ('laptop', 'Laptop screen'),
        ('screenshot', 'Screenshot capture'),
        ('barcode', 'Barcode or QR code tag'),
        ('label', 'Product/equipment label'),
        ('serial number', 'Serial number plate'),
        ('prescription', 'Medical prescription or medication'),
        ('whiteboard', 'Whiteboard notes'),
        ('text on paper', 'Dense printed text'),
    ]

    for kw, reason in doc_indicators:
        if kw in lower_caption:
            return 'DOCUMENT', reason, caption

    return 'PHOTO', 'Authentic visual scene', caption


def classify_with_gemini(
    image_bytes: bytes,
    mime_type: str
) -> Tuple[str, str, str]:
    """Cloud fallback using Gemini Flash-Lite."""
    api_key = os.environ.get('GEMINI_API_KEY') or os.environ.get('GOOGLE_API_KEY')
    if not api_key or not HAS_GENAI:
        return 'PHOTO', 'Default (Gemini unavailable)', ''

    client = _genai.Client(api_key=api_key)
    part = _genai_types.Part.from_bytes(data=image_bytes, mime_type=mime_type)
    prompt = (
        "Classify this image for a personal photo organizer into either 'DOCUMENT' or 'PHOTO'.\n\n"
        "Choose 'DOCUMENT' for receipts, bills, documents, computer/phone screens, product labels.\n"
        "Choose 'PHOTO' for authentic family/personal memories.\n\n"
        "Answer in format: CATEGORY: <DOCUMENT or PHOTO> | REASON: <short 1-sentence reason>"
    )
    try:
        resp = client.models.generate_content(model='gemini-3.1-flash-lite', contents=[part, prompt])
    except Exception:
        resp = client.models.generate_content(model='gemini-3.5-flash-lite', contents=[part, prompt])

    text = resp.text.strip()
    is_doc = 'DOCUMENT' in text.upper()
    cat = 'DOCUMENT' if is_doc else 'PHOTO'
    reason = text.split('| REASON:')[-1].strip() if '| REASON:' in text else ('Document' if is_doc else 'Photo')
    return cat, reason, text


def analyze_image(
    filepath: Path,
    backend: str = "ollama",
    ollama_model: str = "moondream",
    ollama_url: str = "http://127.0.0.1:11434"
) -> Dict[str, Any]:
    """
    Main triage analysis for an image:
    1. Checks cache
    2. Runs fast local filename/aspect heuristics
    3. Runs AI Vision if ambiguous or requested
    Returns classification payload with confidence tier.
    """
    cached = lookup_vision_cache(filepath)
    if cached:
        return {
            "category": cached["category"],
            "tier": "OBVIOUS" if cached.get("confidence", 0.9) > 0.85 else "MIXED",
            "reason": cached["reason"],
            "caption": cached.get("details", ""),
            "is_cached": True
        }

    filename = filepath.name.lower()
    
    # 0. Video files
    if filepath.suffix.lower() in VIDEO_EXTS:
        return {
            "category": "PHOTO",
            "tier": "OBVIOUS",
            "reason": "Video file",
            "caption": "Video recording",
            "is_cached": False
        }

    # 1. Obvious Filename Heuristics
    for pat in DOC_FILENAME_PATTERNS:
        if pat.search(filename):
            res = {
                "category": "DOCUMENT",
                "tier": "OBVIOUS",
                "reason": f"Filename matches keyword: {pat.pattern}",
                "caption": "",
                "is_cached": False
            }
            return res

    # 2. Aspect Ratio Heuristics (e.g. Scrolling Screenshots)
    try:
        with Image.open(filepath) as img:
            w, h = img.size
            aspect = max(w, h) / max(min(w, h), 1)
            if aspect > 2.5:
                return {
                    "category": "DOCUMENT",
                    "tier": "OBVIOUS",
                    "reason": f"Extreme scrolling screenshot aspect ratio ({aspect:.2f})",
                    "caption": "",
                    "is_cached": False
                }
    except Exception:
        pass

    # 3. Vision Pass
    data, mime = get_downscaled_image_bytes(filepath, max_dim=384)
    if not data:
        return {
            "category": "PHOTO",
            "tier": "MIXED",
            "reason": "Unable to decode image bytes",
            "caption": "",
            "is_cached": False
        }

    try:
        if backend == "ollama":
            cat, reason, caption = classify_with_moondream(data, ollama_model, ollama_url)
        else:
            cat, reason, caption = classify_with_gemini(data, mime or 'image/jpeg')

        # Cache the result
        try:
            sz = filepath.stat().st_size
        except Exception:
            sz = 0
        cache_key = f"dc_{filepath.name}_{sz}"
        result_payload = {
            "category": cat,
            "tier": "OBVIOUS" if cat == "DOCUMENT" else "MIXED",
            "reason": reason,
            "details": caption,
            "confidence": 0.95 if cat == "DOCUMENT" else 0.8
        }
        VISION_CACHE[cache_key] = result_payload
        save_vision_cache()

        return {
            "category": cat,
            "tier": result_payload["tier"],
            "reason": reason,
            "caption": caption,
            "is_cached": False
        }
    except Exception as e:
        return {
            "category": "PHOTO",
            "tier": "MIXED",
            "reason": f"Vision error: {str(e)}",
            "caption": "",
            "is_cached": False
        }

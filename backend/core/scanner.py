"""
scanner.py - Filesystem scanner and Takeout metadata extractor for DreamCatcher
Extracts authentic timestamps, GPS coordinates, and pairs Google Takeout JSON sidecars.
"""

import os
import json
import re
from datetime import datetime
from pathlib import Path
from typing import Dict, List, Optional, Tuple, Any, Union
from PIL import Image, ExifTags

try:
    import pillow_heif
    pillow_heif.register_heif_opener()
except ImportError:
    pass

PHOTO_EXTS = {'.jpg', '.jpeg', '.png', '.heic', '.heif', '.webp', '.tiff', '.tif', '.gif'}
VIDEO_EXTS = {'.mp4', '.mov', '.avi', '.m4v', '.mkv'}
MEDIA_EXTS = PHOTO_EXTS | VIDEO_EXTS


def find_json_sidecar(filepath: Path) -> Optional[Path]:
    """
    Finds the Google Takeout JSON sidecar for an image or video file.
    Supports .supplemental-metadata.json, legacy .json, and duplicate (1) variations.
    """
    p_dir = filepath.parent
    name = filepath.name
    stem = filepath.stem
    ext = filepath.suffix

    candidates = [
        p_dir / f"{name}.supplemental-metadata.json",
        p_dir / f"{name}.json",
        p_dir / f"{stem}.supplemental-metadata.json",
        p_dir / f"{stem}.json",
    ]

    # Handle Google Takeout (1) variations e.g. "IMG_001(1).jpg" -> "IMG_001.jpg(1).json"
    m_dup = re.search(r'\((\d+)\)$', stem)
    if m_dup:
        base_stem = stem[:m_dup.start()]
        dup_num = m_dup.group(1)
        candidates.extend([
            p_dir / f"{base_stem}{ext}({dup_num}).supplemental-metadata.json",
            p_dir / f"{base_stem}{ext}({dup_num}).json",
            p_dir / f"{base_stem}({dup_num}){ext}.supplemental-metadata.json",
            p_dir / f"{base_stem}({dup_num}){ext}.json",
        ])

    for c in candidates:
        if c.exists():
            return c
    return None


def extract_timestamp(filepath: Path, sidecar_path: Optional[Path]) -> Tuple[datetime, bool]:
    """
    Extracts the authentic capture timestamp:
    1. Sidecar JSON 'photoTakenTime.timestamp' (highest fidelity)
    2. EXIF DateTimeOriginal / DateTime
    3. Filename YYYYMMDD patterns
    4. Fallback to file creation/modification time
    """
    # 1. Sidecar metadata
    if sidecar_path and sidecar_path.exists():
        try:
            with open(sidecar_path, 'r', encoding='utf-8') as f:
                data = json.load(f)
            ts_str = data.get('photoTakenTime', {}).get('timestamp')
            if ts_str:
                return datetime.fromtimestamp(int(ts_str)), False
        except Exception:
            pass

    # 2. EXIF data for photos
    if filepath.suffix.lower() in PHOTO_EXTS:
        try:
            with Image.open(filepath) as img:
                exif = img.getexif()
                if exif:
                    # Look for DateTimeOriginal (36867) or DateTime (306)
                    for tag_id in (36867, 306):
                        val = exif.get(tag_id)
                        if val and isinstance(val, str):
                            clean = val.strip()
                            if re.match(r'^\d{4}:\d{2}:\d{2} \d{2}:\d{2}:\d{2}$', clean):
                                return datetime.strptime(clean, '%Y:%m:%d %H:%M:%S'), False
        except Exception:
            pass

    # 3. Filename patterns e.g. 20240512_143022 or 2024-05-12
    m = re.search(r'(20\d{2})[-_]?(\d{2})[-_]?(\d{2})', filepath.name)
    if m:
        try:
            y, mo, d = int(m.group(1)), int(m.group(2)), int(m.group(3))
            if 1 <= mo <= 12 and 1 <= d <= 31:
                return datetime(y, mo, d, 12, 0, 0), False
        except Exception:
            pass

    # 4. Fallback: file mtime
    try:
        return datetime.fromtimestamp(filepath.stat().st_mtime), True
    except Exception:
        return datetime.now(), True


def extract_gps_coordinates(filepath: Path, sidecar_path: Optional[Path]) -> Optional[Tuple[float, float]]:
    """Extracts latitude and longitude from sidecar or EXIF."""
    # 1. Takeout JSON sidecar
    if sidecar_path and sidecar_path.exists():
        try:
            with open(sidecar_path, 'r', encoding='utf-8') as f:
                data = json.load(f)
            geo = data.get('geoData', {})
            lat = geo.get('latitude', 0.0)
            lon = geo.get('longitude', 0.0)
            if lat != 0.0 or lon != 0.0:
                return lat, lon
        except Exception:
            pass

    # 2. EXIF GPSInfo tags (tag 34853) for photos
    if filepath.suffix.lower() in PHOTO_EXTS:
        try:
            with Image.open(filepath) as img:
                exif = img.getexif()
                if exif:
                    gps_info = {}
                    if hasattr(exif, 'get_ifd'):
                        try:
                            gps_info = exif.get_ifd(34853)
                        except Exception:
                            pass
                    if not gps_info and hasattr(img, '_getexif'):
                        try:
                            legacy = img._getexif()
                            if legacy and 34853 in legacy:
                                gps_info = legacy[34853]
                        except Exception:
                            pass
                    if not gps_info and 34853 in exif:
                        val = exif[34853]
                        if isinstance(val, dict):
                            gps_info = val

                    if gps_info:
                        normalized = {}
                        for k, v in gps_info.items():
                            name = ExifTags.GPSTAGS.get(k, k) if isinstance(k, int) else k
                            normalized[name] = v

                        lat_data = normalized.get('GPSLatitude')
                        lat_ref = normalized.get('GPSLatitudeRef', 'N')
                        lon_data = normalized.get('GPSLongitude')
                        lon_ref = normalized.get('GPSLongitudeRef', 'E')

                        if lat_data and lon_data:
                            def to_deg(coord, ref):
                                if isinstance(coord, (int, float)):
                                    d = float(coord)
                                elif len(coord) == 3:
                                    d = float(coord[0]) + float(coord[1]) / 60.0 + float(coord[2]) / 3600.0
                                else:
                                    return None
                                if ref and str(ref).strip().upper() in ('S', 'W'):
                                    d = -d
                                return round(d, 6)

                            lat = to_deg(lat_data, lat_ref)
                            lon = to_deg(lon_data, lon_ref)
                            if lat is not None and lon is not None:
                                return (lat, lon)
        except Exception:
            pass

    return None


def scan_directory(
    source_dir: Path,
    month_filter: Optional[str] = None,
    return_stats: bool = False
) -> Union[List[Dict[str, Any]], Tuple[List[Dict[str, Any]], int]]:
    """
    Scans source_dir for all media files and compiles basic file records.
    If month_filter is specified (e.g. '2024-11'), restricts results to items
    matching that calendar month.
    """
    source_dir = source_dir.resolve()
    protected = {'pictures', 'videos', 'pictures_doc', '.git', '_sidecars_archive', 'node_modules'}
    
    target_month = month_filter.strip().replace('_', '-') if month_filter and month_filter.strip() else None
    
    records = []
    total_discovered = 0

    for root, dirs, files in os.walk(source_dir):
        dirs[:] = [d for d in dirs if d.lower() not in protected]
        for f in files:
            p = Path(root) / f
            ext = p.suffix.lower()
            if ext in MEDIA_EXTS:
                total_discovered += 1
                sidecar = find_json_sidecar(p)
                ts, is_undated = extract_timestamp(p, sidecar)
                month_str = ts.strftime('%Y-%m')

                if target_month and month_str != target_month:
                    continue

                gps = extract_gps_coordinates(p, sidecar)
                
                try:
                    size = p.stat().st_size
                except Exception:
                    size = 0

                records.append({
                    "id": f"{p.name}_{size}",
                    "name": p.name,
                    "path": str(p),
                    "ext": ext,
                    "is_video": ext in VIDEO_EXTS,
                    "size": size,
                    "timestamp": ts.isoformat(),
                    "date_str": ts.strftime('%Y-%m-%d'),
                    "month_str": month_str,
                    "is_undated": is_undated,
                    "has_sidecar": sidecar is not None,
                    "sidecar_path": str(sidecar) if sidecar else None,
                    "has_gps": gps is not None,
                    "gps": gps
                })

    records.sort(key=lambda r: r['timestamp'])
    if return_stats:
        return records, total_discovered
    return records

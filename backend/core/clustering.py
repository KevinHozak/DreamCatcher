"""
clustering.py - Chronological event clustering and semantic naming for DreamCatcher
"""

import re
import requests
from datetime import datetime, date
from pathlib import Path
from typing import Dict, List, Optional, Tuple, Any
from .classifier import get_downscaled_image_bytes

GEO_CACHE: Dict[Tuple[float, float], Optional[str]] = {}
LOCAL_CITIES = {'fargo', 'west fargo', 'moorhead', 'dilworth', 'horace', 'harwood'}


def get_holiday_name(d: date) -> Optional[str]:
    year, month, day = d.year, d.month, d.day
    fixed = {
        (1, 1): "New Year's Day",
        (2, 14): "Valentine's Day",
        (3, 17): "St. Patrick's Day",
        (7, 4): "July 4th",
        (10, 31): "Halloween",
        (11, 11): "Veterans Day",
        (12, 24): "Christmas Eve",
        (12, 25): "Christmas",
        (12, 31): "New Year's Eve",
    }
    if (month, day) in fixed:
        return fixed[(month, day)]

    # Easter algorithm
    a = year % 19
    b = year // 100
    c = year % 100
    d_val = b // 4
    e = b % 4
    f = (b + 8) // 25
    g = (b - f + 1) // 3
    h = (19 * a + b - d_val - g + 15) % 30
    i = c // 4
    k = c % 4
    l = (32 + 2 * e + 2 * i - h - k) % 7
    m = (a + 11 * h + 22 * l) // 451
    easter_month = (h + l - 7 * m + 114) // 31
    easter_day = ((h + l - 7 * m + 114) % 31) + 1
    if month == easter_month and day == easter_day:
        return "Easter"

    # Thanksgiving (4th Thursday of November)
    if month == 11 and date(year, 11, day).weekday() == 3:
        if 22 <= day <= 28:
            return "Thanksgiving"

    return None


def reverse_geocode_city(lat: float, lon: float) -> Optional[str]:
    key = (round(lat, 3), round(lon, 3))
    if key in GEO_CACHE:
        return GEO_CACHE[key]

    try:
        url = "https://nominatim.openstreetmap.org/reverse"
        params = {"lat": lat, "lon": lon, "format": "json", "zoom": 12, "addressdetails": 1}
        headers = {"User-Agent": "DreamCatcher-PhotoTriage/1.0"}
        resp = requests.get(url, params=params, headers=headers, timeout=5)
        if resp.status_code == 200:
            addr = resp.json().get('address', {})
            city = addr.get('city') or addr.get('town') or addr.get('village') or addr.get('municipality')
            if city:
                GEO_CACHE[key] = city
                return city
    except Exception:
        pass

    GEO_CACHE[key] = None
    return None


def cluster_items(
    items: List[Dict[str, Any]],
    cluster_hours: float = 4.0,
    min_cluster_size: int = 5
) -> List[Dict[str, Any]]:
    """
    Groups family media items chronologically into clusters.
    Returns list of clusters:
    {
       "id": "cluster_YYYY-MM-DD_idx",
       "date_str": "YYYY-MM-DD",
       "folder_name": "(YYYY-MM-DD) Description",
       "is_holiday": bool,
       "items": [...]
    }
    """
    if not items:
        return []

    # Sort items by timestamp
    sorted_items = sorted(items, key=lambda x: x['timestamp'])

    # Group by calendar date
    by_date: Dict[str, List[Dict[str, Any]]] = {}
    for it in sorted_items:
        d = it['date_str']
        by_date.setdefault(d, []).append(it)

    clusters_out = []

    for d_str, day_items in by_date.items():
        cal_date = datetime.strptime(d_str, '%Y-%m-%d').date()
        holiday = get_holiday_name(cal_date)

        if holiday:
            clusters_out.append({
                "id": f"cluster_{d_str}_holiday",
                "date_str": d_str,
                "folder_name": f"({d_str}) {holiday}",
                "is_holiday": True,
                "is_event": True,
                "items": day_items
            })
            continue

        # Split day items by time delta
        sub_clusters = []
        curr = [day_items[0]]
        for it in day_items[1:]:
            prev_dt = datetime.fromisoformat(curr[-1]['timestamp'])
            curr_dt = datetime.fromisoformat(it['timestamp'])
            if (curr_dt - prev_dt).total_seconds() / 3600.0 <= cluster_hours:
                curr.append(it)
            else:
                sub_clusters.append(curr)
                curr = [it]
        if curr:
            sub_clusters.append(curr)

        event_groups = [c for c in sub_clusters if len(c) >= min_cluster_size]
        loose_items = [it for c in sub_clusters if len(c) < min_cluster_size for it in c]

        for idx, eg in enumerate(event_groups):
            # Location
            city = None
            for it in eg:
                if it.get('gps'):
                    city = reverse_geocode_city(it['gps'][0], it['gps'][1])
                    if city:
                        break

            label = f"{city} - Event" if city and city.lower() not in LOCAL_CITIES else "Event Candidate"
            clusters_out.append({
                "id": f"cluster_{d_str}_{idx}",
                "date_str": d_str,
                "folder_name": f"({d_str}) {label}",
                "is_holiday": False,
                "is_event": True,
                "items": eg
            })

        if loose_items:
            clusters_out.append({
                "id": f"cluster_{d_str}_daily",
                "date_str": d_str,
                "folder_name": f"({d_str}) Daily Life",
                "is_holiday": False,
                "is_event": False,
                "items": loose_items
            })

    return clusters_out

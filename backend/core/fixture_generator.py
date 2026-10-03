"""Deterministic synthetic Google Photos Takeout media test fixture generator."""

import argparse
import hashlib
import io
import json
import random
import shutil
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Optional

from PIL import Image


def _utc_iso(dt: datetime) -> str:
    return dt.astimezone(timezone.utc).isoformat()


def compute_sha256(path: Path) -> str:
    """Computes SHA-256 hex digest for a file."""
    h = hashlib.sha256()
    with path.open("rb") as handle:
        while chunk := handle.read(65536):
            h.update(chunk)
    return h.hexdigest()


def create_synthetic_jpeg(
    path: Path,
    width: int = 320,
    height: int = 240,
    color: tuple[int, int, int] = (120, 160, 200),
    date_time: Optional[datetime] = None,
    unique_tag: str = "",
) -> int:
    """Generates a valid JPEG file with authentic EXIF DateTime tags."""
    path.parent.mkdir(parents=True, exist_ok=True)
    img = Image.new("RGB", (width, height), color=color)

    exif = img.getexif()
    if date_time:
        exif_dt = date_time.strftime("%Y:%m:%d %H:%M:%S")
        exif[306] = exif_dt  # DateTime
        exif[36867] = exif_dt  # DateTimeOriginal
        exif[36868] = exif_dt  # DateTimeDigitized
    if unique_tag:
        exif[270] = f"DreamCatcher Test: {unique_tag}"  # ImageDescription

    img.save(path, format="JPEG", quality=90, exif=exif)
    return path.stat().st_size


def create_synthetic_mp4(
    path: Path,
    duration_sec: int = 5,
    payload_content: bytes = b"DREAMCATCHER_SYNTHETIC_VIDEO_DATA",
) -> int:
    """Generates a valid ISO Base Media File Format (MP4) binary with ftyp, moov, and mdat boxes."""
    path.parent.mkdir(parents=True, exist_ok=True)

    # 1. ftyp box (32 bytes)
    ftyp = b"\x00\x00\x00\x20ftypisom\x00\x00\x02\x00isomiso2mp41"

    # 2. moov -> mvhd atom box
    mvhd_payload = (
        b"\x00\x00\x00\x00"  # version (0) and flags
        + b"\x00\x00\x00\x00"  # creation time
        + b"\x00\x00\x00\x00"  # modification time
        + b"\x00\x00\x03\xe8"  # timescale (1000 Hz)
        + duration_sec.to_bytes(4, "big")  # duration
        + b"\x00\x01\x00\x00"  # rate 1.0
        + b"\x01\x00"  # volume 1.0
        + b"\x00" * 10  # reserved
        + b"\x00\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00"
        + b"\x00\x01\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00"
        + b"\x40\x00\x00\x00"  # matrix structure
        + b"\x00" * 24  # pre-defined
        + b"\x00\x00\x00\x02"  # next track id
    )
    mvhd_len = len(mvhd_payload) + 8
    mvhd = mvhd_len.to_bytes(4, "big") + b"mvhd" + mvhd_payload
    moov_len = len(mvhd) + 8
    moov = moov_len.to_bytes(4, "big") + b"moov" + mvhd

    # 3. mdat box
    mdat_len = len(payload_content) + 8
    mdat = mdat_len.to_bytes(4, "big") + b"mdat" + payload_content

    full_bytes = ftyp + moov + mdat
    path.write_bytes(full_bytes)
    return len(full_bytes)


def create_takeout_sidecar(
    media_path: Path,
    sidecar_format: str = "supplemental",
    title: Optional[str] = None,
    description: str = "",
    date_time: Optional[datetime] = None,
    lat: Optional[float] = None,
    lon: Optional[float] = None,
    people: Optional[list[str]] = None,
) -> Path:
    """
    Creates a paired Takeout JSON metadata sidecar:
    - 'supplemental': <filename>.supplemental-metadata.json
    - 'short': <filename>.json
    """
    if sidecar_format == "supplemental":
        sidecar_path = media_path.parent / f"{media_path.name}.supplemental-metadata.json"
    else:
        sidecar_path = media_path.parent / f"{media_path.name}.json"

    dt = date_time or datetime.now(timezone.utc)
    ts = str(int(dt.timestamp()))
    formatted_time = dt.strftime("%b %d, %Y, %I:%M:%S %p UTC")

    geo_data = {
        "latitude": lat if lat is not None else 0.0,
        "longitude": lon if lon is not None else 0.0,
        "altitude": 10.0 if (lat or lon) else 0.0,
        "latitudeSpan": 0.0,
        "longitudeSpan": 0.0,
    }

    payload = {
        "title": title or media_path.name,
        "description": description,
        "imageViews": "1",
        "creationTime": {"timestamp": ts, "formatted": formatted_time},
        "photoTakenTime": {"timestamp": ts, "formatted": formatted_time},
        "geoData": geo_data,
        "geoDataExif": geo_data,
        "people": [{"name": p} for p in (people or [])],
        "googlePhotosOrigin": {"mobileUpload": {"deviceType": "ANDROID_PHONE"}},
    }

    sidecar_path.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    return sidecar_path


def generate_synthetic_takeout_fixture(
    target_dir: Path,
    seed: int = 42,
    clean: bool = False,
    generate_destinations: bool = True,
    generate_legacy_inventory: bool = True,
    generated_at: Optional[str] = None,
) -> dict:
    """
    Deterministically generates a complete synthetic Google Photos Takeout media fixture
    in target_dir, complete with source files, sidecars, duplicates, destination collision targets,
    legacy inventory store, and a comprehensive fixture_manifest.json.
    """
    target_dir = Path(target_dir).resolve()
    if clean and target_dir.exists():
        shutil.rmtree(target_dir, ignore_errors=True)

    target_dir.mkdir(parents=True, exist_ok=True)
    rng = random.Random(seed)

    # 1. Define directory tree
    takeout_dir = target_dir / "Takeout" / "Google Photos"
    folder_2024 = takeout_dir / "Photos from 2024"
    folder_trip = takeout_dir / "Trip to Mountains"
    folder_docs = takeout_dir / "Receipts and Docs"

    folder_2024.mkdir(parents=True, exist_ok=True)
    folder_trip.mkdir(parents=True, exist_ok=True)
    folder_docs.mkdir(parents=True, exist_ok=True)

    pictures_dest = target_dir / "Pictures"
    videos_dest = target_dir / "Videos"
    if generate_destinations:
        (pictures_dest / "Daily Life").mkdir(parents=True, exist_ok=True)
        (pictures_dest / "Trip to Mountains").mkdir(parents=True, exist_ok=True)
        (videos_dest / "Daily Life").mkdir(parents=True, exist_ok=True)

    manifest_files: list[dict] = []
    duplicate_groups: dict[str, list[str]] = {}
    expected_collisions: list[dict] = []

    def record_file(p: Path, kind: str, category: str, sidecar_p: Optional[Path] = None):
        h = compute_sha256(p)
        rel = str(p.relative_to(target_dir)).replace("\\", "/")
        sc_rel = str(sidecar_p.relative_to(target_dir)).replace("\\", "/") if sidecar_p else None
        item = {
            "path": rel,
            "filename": p.name,
            "kind": kind,
            "category": category,
            "size_bytes": p.stat().st_size,
            "sha256": h,
            "has_sidecar": sidecar_p is not None,
            "sidecar_path": sc_rel,
        }
        manifest_files.append(item)
        if kind in {"photo", "video"}:
            duplicate_groups.setdefault(h, []).append(rel)
        return item

    # 2. Source Photos from 2024
    # Photo 1: Standard photo with supplemental metadata sidecar & GPS (will collide with destination)
    dt1 = datetime(2024, 1, 15, 10, 30, 0, tzinfo=timezone.utc)
    p1 = folder_2024 / "IMG_20240115_001.jpg"
    create_synthetic_jpeg(p1, 400, 300, (140, 180, 220), dt1, "Photo 2024-01-15 #1")
    sc1 = create_takeout_sidecar(
        p1,
        sidecar_format="supplemental",
        title="IMG_20240115_001.jpg",
        description="Morning walk at sunrise",
        date_time=dt1,
        lat=37.7749,
        lon=-122.4194,
        people=["Alice"],
    )
    record_file(p1, "photo", "PHOTO", sc1)
    record_file(sc1, "sidecar", "METADATA")

    # Photo 2: Photo with standard short .json sidecar
    dt2 = datetime(2024, 1, 16, 14, 0, 0, tzinfo=timezone.utc)
    p2 = folder_2024 / "IMG_20240116_002.jpg"
    create_synthetic_jpeg(p2, 400, 300, (180, 140, 100), dt2, "Photo 2024-01-16 #2")
    sc2 = create_takeout_sidecar(
        p2,
        sidecar_format="short",
        title="IMG_20240116_002.jpg",
        description="Afternoon coffee",
        date_time=dt2,
        lat=37.7833,
        lon=-122.4167,
    )
    record_file(p2, "photo", "PHOTO", sc2)
    record_file(sc2, "sidecar", "METADATA")

    # Photo 3: Photo with multiple collisions in destination (triggers _2)
    dt3 = datetime(2024, 1, 18, 9, 15, 0, tzinfo=timezone.utc)
    p3 = folder_2024 / "IMG_20240118_COLLIDE2.jpg"
    create_synthetic_jpeg(p3, 400, 300, (200, 100, 120), dt3, "Multi-Collision Photo")
    record_file(p3, "photo", "PHOTO")

    # Video 1: MP4 Video (will collide with destination)
    v1 = folder_2024 / "VID_20240115_001.mp4"
    create_synthetic_mp4(v1, duration_sec=6, payload_content=b"MP4_VIDEO_STREAM_2024_01_15")
    sc_v1 = create_takeout_sidecar(
        v1,
        sidecar_format="supplemental",
        title="VID_20240115_001.mp4",
        description="Video clip at the park",
        date_time=dt1,
    )
    record_file(v1, "video", "VIDEO", sc_v1)
    record_file(sc_v1, "sidecar", "METADATA")

    # Video 2: MP4 Video without sidecar
    dt_v2 = datetime(2024, 1, 20, 16, 45, 0, tzinfo=timezone.utc)
    v2 = folder_2024 / "VID_20240120_002.mp4"
    create_synthetic_mp4(v2, duration_sec=10, payload_content=b"MP4_VIDEO_STREAM_2024_01_20")
    record_file(v2, "video", "VIDEO")

    # 3. Duplicate Candidate Cluster (Identical byte clones)
    # Generate authentic valid JPEG bytes
    dup_img = Image.new("RGB", (320, 240), color=(150, 100, 200))
    dup_io = io.BytesIO()
    dup_img.save(dup_io, format="JPEG", quality=90)
    dup_bytes = dup_io.getvalue()

    dup_a = folder_2024 / "IMG_20240125_DUP_A.jpg"
    dup_b = folder_2024 / "IMG_20240125_DUP_B.jpg"
    dup_a.write_bytes(dup_bytes)
    dup_b.write_bytes(dup_bytes)
    record_file(dup_a, "photo", "PHOTO")
    record_file(dup_b, "photo", "PHOTO")

    # Duplicate C in Trip to Mountains (Cross-folder duplicate clone)
    dup_c = folder_trip / "IMG_20240205_DUP_C.jpg"
    dup_c.write_bytes(dup_bytes)
    record_file(dup_c, "photo", "PHOTO")

    # 4. Trip to Mountains Folder
    dt_trip1 = datetime(2024, 2, 1, 11, 0, 0, tzinfo=timezone.utc)
    p_trip1 = folder_trip / "IMG_20240201_TRIP1.jpg"
    create_synthetic_jpeg(p_trip1, 400, 300, (80, 160, 90), dt_trip1, "Mountain Trail Summit")
    sc_trip1 = create_takeout_sidecar(
        p_trip1,
        sidecar_format="supplemental",
        title="IMG_20240201_TRIP1.jpg",
        description="Summit ridge trail view",
        date_time=dt_trip1,
        lat=39.7392,
        lon=-104.9903,
        people=["Alice", "Bob"],
    )
    record_file(p_trip1, "photo", "PHOTO", sc_trip1)
    record_file(sc_trip1, "sidecar", "METADATA")

    dt_trip2 = datetime(2024, 2, 2, 15, 30, 0, tzinfo=timezone.utc)
    v_trip1 = folder_trip / "VID_20240202_TRIP1.mp4"
    create_synthetic_mp4(v_trip1, duration_sec=8, payload_content=b"MOUNTAIN_RIVER_STREAM_2024")
    record_file(v_trip1, "video", "VIDEO")

    # 5. Documents / Receipts Folder
    doc1 = folder_docs / "20240301_Store_Receipt.png"
    # Simple valid PNG image
    img_doc = Image.new("RGB", (300, 400), color=(245, 245, 240))
    img_doc.save(doc1, format="PNG")
    record_file(doc1, "photo", "DOCUMENT")

    # 6. Pre-existing files in Destination Folders (Collision Targets)
    if generate_destinations:
        # Pre-existing file in Pictures/Daily Life to collide with IMG_20240115_001.jpg
        dest_p1 = pictures_dest / "Daily Life" / "IMG_20240115_001.jpg"
        dest_p1_img = Image.new("RGB", (320, 240), color=(50, 50, 50))
        dest_p1_img.save(dest_p1, format="JPEG", quality=90)
        dest_sc1 = pictures_dest / "Daily Life" / "IMG_20240115_001.jpg.supplemental-metadata.json"
        dest_sc1.write_text(json.dumps({"title": "Existing Destination Sidecar"}, indent=2), encoding="utf-8")
        record_file(dest_p1, "destination_collision", "PHOTO")
        record_file(dest_sc1, "destination_sidecar", "METADATA")

        expected_collisions.append({
            "source": str(p1.relative_to(target_dir)).replace("\\", "/"),
            "target_intended": "Pictures/Daily Life/IMG_20240115_001.jpg",
            "expected_resolved_destination": "Pictures/Daily Life/IMG_20240115_001_1.jpg",
            "expected_sidecar_resolved": "Pictures/Daily Life/IMG_20240115_001_1.jpg.supplemental-metadata.json",
        })

        # Pre-existing files in Pictures/Daily Life to trigger collision avoidance twice (_1 and _2)
        dest_c1 = pictures_dest / "Daily Life" / "IMG_20240118_COLLIDE2.jpg"
        dest_c2 = pictures_dest / "Daily Life" / "IMG_20240118_COLLIDE2_1.jpg"
        Image.new("RGB", (320, 240), color=(60, 60, 60)).save(dest_c1, format="JPEG", quality=90)
        Image.new("RGB", (320, 240), color=(70, 70, 70)).save(dest_c2, format="JPEG", quality=90)
        record_file(dest_c1, "destination_collision", "PHOTO")
        record_file(dest_c2, "destination_collision", "PHOTO")

        expected_collisions.append({
            "source": str(p3.relative_to(target_dir)).replace("\\", "/"),
            "target_intended": "Pictures/Daily Life/IMG_20240118_COLLIDE2.jpg",
            "expected_resolved_destination": "Pictures/Daily Life/IMG_20240118_COLLIDE2_2.jpg",
        })

        # Pre-existing file in Videos/Daily Life to collide with VID_20240115_001.mp4
        dest_v1 = videos_dest / "Daily Life" / "VID_20240115_001.mp4"
        create_synthetic_mp4(dest_v1, duration_sec=3, payload_content=b"EXISTING_VIDEO_DESTINATION_FILE_KEEP_SAFE")
        record_file(dest_v1, "destination_collision", "VIDEO")

        expected_collisions.append({
            "source": str(v1.relative_to(target_dir)).replace("\\", "/"),
            "target_intended": "Videos/Daily Life/VID_20240115_001.mp4",
            "expected_resolved_destination": "Videos/Daily Life/VID_20240115_001_1.mp4",
            "expected_sidecar_resolved": "Videos/Daily Life/VID_20240115_001_1.mp4.supplemental-metadata.json",
        })

    # 7. Mock Legacy inventory.json
    legacy_json_path = target_dir / "inventory.json"
    if generate_legacy_inventory:
        legacy_records = [
            {
                "identity": f"legacy-record-{idx}",
                "path": item["path"],
                "root_kind": "pictures" if item["kind"] == "photo" else "videos",
                "media_type": "picture" if item["kind"] == "photo" else "video",
                "extension": Path(item["path"]).suffix,
                "size": item["size_bytes"],
                "timestamp": _utc_iso(dt1),
                "is_undated": False,
                "has_sidecar": item["has_sidecar"],
                "has_gps": True if item["has_sidecar"] else False,
                "last_seen_scan": "scan-legacy-synthetic-01",
                "state": "available",
            }
            for idx, item in enumerate(manifest_files)
            if item["kind"] in {"photo", "video"}
        ]
        legacy_scans = [
            {
                "scan_id": "scan-legacy-synthetic-01",
                "root_kind": "pictures",
                "root_path": "Takeout/Google Photos",
                "status": "completed",
                "discovered": len(legacy_records),
                "indexed": len(legacy_records),
                "skipped": 0,
                "completed_at": _utc_iso(dt1),
            }
        ]
        legacy_payload = {"records": legacy_records, "scans": legacy_scans}
        legacy_json_path.write_text(json.dumps(legacy_payload, indent=2), encoding="utf-8")
        record_file(legacy_json_path, "legacy_inventory", "DATABASE")

    # 8. Filter duplicate groups to only those with >= 2 items
    multi_duplicates = {k: v for k, v in duplicate_groups.items() if len(v) > 1}

    # 9. Build Manifest
    counts = {
        "total_files": len(manifest_files),
        "photos": sum(1 for f in manifest_files if f["kind"] == "photo"),
        "videos": sum(1 for f in manifest_files if f["kind"] == "video"),
        "sidecars": sum(1 for f in manifest_files if f["kind"] in {"sidecar", "destination_sidecar"}),
        "destination_files": sum(1 for f in manifest_files if f["kind"] in {"destination_collision", "destination_sidecar"}),
        "duplicate_sets": len(multi_duplicates),
        "duplicate_items": sum(len(v) for v in multi_duplicates.values()),
        "legacy_inventory_records": len(legacy_records) if generate_legacy_inventory else 0,
    }

    manifest = {
        "generator_version": "1.0.0",
        "seed": seed,
        "generated_at": generated_at or "2024-01-15T12:00:00+00:00",
        "counts": counts,
        "duplicate_groups": multi_duplicates,
        "expected_collisions": expected_collisions,
        "files": manifest_files,
    }

    manifest_path = target_dir / "fixture_manifest.json"
    manifest_path.write_text(json.dumps(manifest, indent=2), encoding="utf-8")
    return manifest


def main():
    parser = argparse.ArgumentParser(description="Generate synthetic Google Photos Takeout media fixture")
    parser.add_argument("--target", "-t", required=True, help="Target directory for the fixture")
    parser.add_argument("--seed", "-s", type=int, default=42, help="Deterministic random seed (default: 42)")
    parser.add_argument("--clean", "-c", action="store_true", help="Clean target directory before generation")
    parser.add_argument("--no-destinations", action="store_true", help="Do not create pre-existing destination collision files")
    parser.add_argument("--no-legacy", action="store_true", help="Do not create mock legacy inventory.json")
    args = parser.parse_args()

    target = Path(args.target)
    print(f"Generating synthetic Takeout fixture in: {target} (seed={args.seed})...")
    manifest = generate_synthetic_takeout_fixture(
        target_dir=target,
        seed=args.seed,
        clean=args.clean,
        generate_destinations=not args.no_destinations,
        generate_legacy_inventory=not args.no_legacy,
    )
    counts = manifest["counts"]
    print("Generation complete! Summary:")
    print(f" - Total files generated: {counts['total_files']}")
    print(f" - Photos: {counts['photos']}")
    print(f" - Videos: {counts['videos']}")
    print(f" - Sidecars: {counts['sidecars']}")
    print(f" - Destination collision files: {counts['destination_files']}")
    print(f" - Duplicate candidate sets: {counts['duplicate_sets']} ({counts['duplicate_items']} files)")
    print(f" - Manifest written to: {target / 'fixture_manifest.json'}")


if __name__ == "__main__":
    main()

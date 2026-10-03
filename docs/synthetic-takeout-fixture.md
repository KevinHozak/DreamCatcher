# 📸 Reproducible Synthetic Takeout Media Test Fixture

This document describes the design, generation, and structure of the reproducible synthetic Google Photos Takeout test fixture used for automated and interactive desktop verification in DreamCatcher.

---

## 🎯 Purpose

Desktop release verification requires an isolated, non-destructive test library replicating real-world Google Photos Takeout archives without relying on personal user data.

The synthetic fixture generator creates:
- Valid JPEG photos with authentic EXIF DateTime tags (`DateTimeOriginal` and `DateTime`).
- Valid MP4 video containers with standard ISO base media file format headers (`ftyp`, `moov`, `mdat`).
- Paired Google Photos JSON metadata sidecars (`.json` and `.supplemental-metadata.json`).
- Pre-existing files in destination folders (`Pictures/`, `Videos/`) to exercise collision avoidance (`_1`, `_2`) and rollback restoration.
- Cross-folder duplicate media candidates with identical SHA-256 byte hashes.
- Mock legacy `inventory.json` database files to test first-open migration and backup preservation.
- A comprehensive machine-readable `fixture_manifest.json` recording exact file counts, sizes, hashes, and expected collision resolutions.

---

## 🛠️ Generating the Fixture

### 1. Via Python CLI Script
Generate a fixture in any target directory (e.g. temporary directory or external transfer disk):

```powershell
python scripts/generate_takeout_fixture.py --target "C:\Transfer\Takeout_Fixture" --clean
```

### 2. Custom Seed & Options
```powershell
# Custom deterministic seed
python scripts/generate_takeout_fixture.py --target "./test_fixture" --seed 42

# Generate without destination collision files
python scripts/generate_takeout_fixture.py --target "./test_fixture" --no-destinations

# Generate without legacy inventory.json
python scripts/generate_takeout_fixture.py --target "./test_fixture" --no-legacy
```

### 3. Programmatic Usage (Python Tests)
```python
from pathlib import Path
from core.fixture_generator import generate_synthetic_takeout_fixture

manifest = generate_synthetic_takeout_fixture(
    target_dir=Path("C:/Transfer/Takeout_Fixture"),
    seed=42,
    clean=True,
)
print(f"Generated {manifest['counts']['total_files']} files")
```

---

## 📂 Fixture Directory Structure

```text
<fixture_root>/
├── fixture_manifest.json                           # Machine-readable inventory & collision manifest
├── inventory.json                                  # Mock legacy inventory for migration testing
├── Takeout/
│   └── Google Photos/
│       ├── Photos from 2024/
│       │   ├── IMG_20240115_001.jpg                # Photo 1 (Collides with destination)
│       │   ├── IMG_20240115_001.jpg.supplemental-metadata.json
│       │   ├── IMG_20240116_002.jpg                # Photo 2 (Paired with short .json)
│       │   ├── IMG_20240116_002.jpg.json
│       │   ├── IMG_20240118_COLLIDE2.jpg           # Photo 3 (Collides twice in destination -> _2)
│       │   ├── IMG_20240125_DUP_A.jpg              # Duplicate copy A
│       │   ├── IMG_20240125_DUP_B.jpg              # Duplicate copy B
│       │   ├── VID_20240115_001.mp4                # Video 1 (Collides with destination)
│       │   ├── VID_20240115_001.mp4.supplemental-metadata.json
│       │   └── VID_20240120_002.mp4                # Video 2 (No sidecar)
│       ├── Trip to Mountains/
│       │   ├── IMG_20240201_TRIP1.jpg              # Photo with GPS & People metadata
│       │   ├── IMG_20240201_TRIP1.jpg.supplemental-metadata.json
│       │   ├── IMG_20240205_DUP_C.jpg              # Duplicate copy C (Cross-folder clone of A & B)
│       │   └── VID_20240202_TRIP1.mp4              # Trip Video
│       └── Receipts and Docs/
│           └── 20240301_Store_Receipt.png          # Document / Receipt triage test
├── Pictures/                                       # Configured Destination Root
│   ├── Daily Life/
│   │   ├── IMG_20240115_001.jpg                    # Pre-existing file (forces _1)
│   │   ├── IMG_20240115_001.jpg.supplemental-metadata.json
│   │   ├── IMG_20240118_COLLIDE2.jpg               # Pre-existing file
│   │   └── IMG_20240118_COLLIDE2_1.jpg             # Pre-existing file (forces _2)
│   └── Trip to Mountains/
└── Videos/                                         # Configured Destination Root
    └── Daily Life/
        └── VID_20240115_001.mp4                    # Pre-existing video (forces _1)
```

---

## 📊 Documented Counts & Metrics (Default Seed 42)

| Category | File Count | Description |
| :--- | :---: | :--- |
| **Total Files** | **21** | Total generated files in fixture root |
| **Photos** | **8** | Valid JPEG & PNG images across source directories |
| **Videos** | **3** | Valid ISO Base MP4 video files |
| **Sidecars** | **5** | Google Photos Takeout metadata JSON sidecars |
| **Destination Pre-existing** | **5** | Pre-existing destination files for collision testing |
| **Duplicate Candidate Sets** | **1** | 1 set of 3 identical byte clones (`DUP_A`, `DUP_B`, `DUP_C`) |
| **Legacy Inventory Records** | **11** | Pre-indexed records in `inventory.json` |

---

## 💥 Documented Collision Paths

When triaging the source Takeout media into the configured destination directories, DreamCatcher's collision avoidance logic must preserve existing destination files and append `_1` or `_2` suffixes.

| Source File | Intended Target | Pre-existing Conflict | Expected Resolved Destination | Expected Resolved Sidecar |
| :--- | :--- | :--- | :--- | :--- |
| `IMG_20240115_001.jpg` | `Pictures/Daily Life/` | `IMG_20240115_001.jpg` exists | `Pictures/Daily Life/IMG_20240115_001_1.jpg` | `IMG_20240115_001_1.jpg.supplemental-metadata.json` |
| `IMG_20240118_COLLIDE2.jpg` | `Pictures/Daily Life/` | Both `IMG...jpg` and `..._1.jpg` exist | `Pictures/Daily Life/IMG_20240118_COLLIDE2_2.jpg` | N/A |
| `VID_20240115_001.mp4` | `Videos/Daily Life/` | `VID_20240115_001.mp4` exists | `Videos/Daily Life/VID_20240115_001_1.mp4` | `VID_20240115_001_1.mp4.supplemental-metadata.json` |

---

## 🔄 Duplicate Media Groups

The fixture deterministically embeds an exact duplicate group consisting of 3 identical files sharing the exact same SHA-256 hash (`dbcfe5997154d82f14e413291c9d24815d4faef5c2ee7f7e692f14486d2c52e0`):
1. `Takeout/Google Photos/Photos from 2024/IMG_20240125_DUP_A.jpg`
2. `Takeout/Google Photos/Photos from 2024/IMG_20240125_DUP_B.jpg`
3. `Takeout/Google Photos/Trip to Mountains/IMG_20240205_DUP_C.jpg`

Used to verify:
- Exact duplicate detection across subdirectories.
- Review-only non-destructive behavior (zero files deleted).
- Member exclusion persistence.

---

## 🧪 Automated Test Coverage

The fixture generator is covered by tests in [`backend/tests/test_fixture_generator.py`](../backend/tests/test_fixture_generator.py):
- `test_generator_determinism_and_reproducibility`: Confirms identical bit-for-bit file generation across runs with the same seed.
- `test_media_format_and_sidecar_validity`: Validates JPEG EXIF headers, MP4 boxes, and Takeout JSON sidecar schemas.
- `test_triage_collision_handling_and_rollback_with_fixture`: Executes triage against the fixture, confirms `_1` and `_2` collision resolutions, and verifies full rollback restoration.
- `test_duplicate_candidate_detection_on_fixture`: Scans the fixture into inventory and confirms detection of the duplicate triplet group.

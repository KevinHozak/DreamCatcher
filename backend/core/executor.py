"""
executor.py - Safe media mover and sidecar relocation engine for DreamCatcher
"""

import os
import shutil
import json
from pathlib import Path
from typing import Dict, List, Any, Tuple


def get_unique_destination_path(target_path: Path) -> Path:
    """
    If target_path already exists, appends _1, _2, etc. before the extension
    to avoid overwriting existing files. Handles double extensions like .supplemental-metadata.json.
    """
    if not target_path.exists():
        return target_path

    p_dir = target_path.parent
    name = target_path.name

    if name.endswith('.supplemental-metadata.json'):
        base = name[:-len('.supplemental-metadata.json')]
        ext = '.supplemental-metadata.json'
    else:
        base = target_path.stem
        ext = target_path.suffix

    counter = 1
    new_path = p_dir / f"{base}_{counter}{ext}"
    while new_path.exists():
        counter += 1
        new_path = p_dir / f"{base}_{counter}{ext}"
    return new_path


def execute_triage_plan(
    source_dir: Path,
    triage_decisions: Dict[str, Dict[str, Any]],
    action: str = "move"
) -> Dict[str, Any]:
    """
    Executes triage decisions:
    - 'FAMILY' -> source_dir / ('Videos' if is_video else 'Pictures') / folder_name / file
    - 'DOCUMENT' -> source_dir / 'Pictures_Doc' / YYYY-MM / file
    - Moves all paired sidecars alongside each file.
    - Implements collision avoidance check to avoid overwriting existing files.
    Writes a rollback ledger to source_dir / '_dreamcatcher_ledger.json'.
    """
    source_dir = source_dir.resolve()
    pics_dir = source_dir / 'Pictures'
    vids_dir = source_dir / 'Videos'
    docs_dir = source_dir / 'Pictures_Doc'

    ledger_entries = []
    success_count = 0
    error_count = 0

    for file_id, info in triage_decisions.items():
        src_path = Path(info['path'])
        if not src_path.exists():
            continue

        category = info.get('category', 'FAMILY')
        is_video = info.get('is_video', False)
        dest_folder = info.get('folder_name', 'Daily Life')
        month_str = info.get('month_str', 'General')

        if category == 'DOCUMENT':
            target_dir = docs_dir / month_str
        else:
            target_dir = (vids_dir if is_video else pics_dir) / dest_folder

        target_dir.mkdir(parents=True, exist_ok=True)
        dest_file = get_unique_destination_path(target_dir / src_path.name)

        try:
            # 1. Move/Copy main media file
            if action == 'move':
                shutil.move(str(src_path), str(dest_file))
            else:
                shutil.copy2(str(src_path), str(dest_file))

            item_ledger = {
                "id": file_id,
                "src": str(src_path),
                "dest": str(dest_file),
                "sidecars": []
            }

            # 2. Relocate sidecars if present
            sidecar_path = info.get('sidecar_path')
            candidates = []
            if sidecar_path and Path(sidecar_path).exists():
                candidates.append(Path(sidecar_path))
            else:
                p_dir = src_path.parent
                candidates.extend([
                    p_dir / f"{src_path.name}.supplemental-metadata.json",
                    p_dir / f"{src_path.name}.json",
                    p_dir / f"{src_path.stem}.supplemental-metadata.json",
                    p_dir / f"{src_path.stem}.json"
                ])

            for sc in candidates:
                if sc.exists():
                    # If main media file was renamed due to collision, preserve pairing for sidecar
                    if dest_file.name != src_path.name:
                        if sc.name == f"{src_path.name}.supplemental-metadata.json":
                            dest_sc = target_dir / f"{dest_file.name}.supplemental-metadata.json"
                        elif sc.name == f"{src_path.name}.json":
                            dest_sc = target_dir / f"{dest_file.name}.json"
                        elif sc.name == f"{src_path.stem}.supplemental-metadata.json":
                            dest_sc = target_dir / f"{dest_file.stem}.supplemental-metadata.json"
                        elif sc.name == f"{src_path.stem}.json":
                            dest_sc = target_dir / f"{dest_file.stem}.json"
                        else:
                            dest_sc = target_dir / sc.name
                    else:
                        dest_sc = target_dir / sc.name

                    dest_sc = get_unique_destination_path(dest_sc)

                    if action == 'move':
                        shutil.move(str(sc), str(dest_sc))
                    else:
                        shutil.copy2(str(sc), str(dest_sc))
                    item_ledger["sidecars"].append({"src": str(sc), "dest": str(dest_sc)})

            ledger_entries.append(item_ledger)
            success_count += 1
        except Exception as e:
            error_count += 1

    # Save ledger
    ledger_file = source_dir / "_dreamcatcher_ledger.json"
    with open(ledger_file, 'w', encoding='utf-8') as f:
        json.dump({
            "timestamp": str(os.stat(source_dir).st_mtime),
            "action": action,
            "total_success": success_count,
            "total_errors": error_count,
            "entries": ledger_entries
        }, f, indent=2)

    return {
        "success": True,
        "moved": success_count,
        "errors": error_count,
        "ledger": str(ledger_file)
    }


def rollback_triage_plan(source_dir: Path) -> Dict[str, Any]:
    """
    Rolls back the most recent triage run using source_dir / '_dreamcatcher_ledger.json'.
    Restores files and sidecars to their original pre-triage locations.
    """
    source_dir = source_dir.resolve()
    ledger_file = source_dir / "_dreamcatcher_ledger.json"
    if not ledger_file.exists():
        raise FileNotFoundError(f"No triage ledger found at {ledger_file}")

    with open(ledger_file, 'r', encoding='utf-8') as f:
        ledger_data = json.load(f)

    action = ledger_data.get("action", "move")
    entries = ledger_data.get("entries", [])

    restored_items = 0
    restored_sidecars = 0
    errors = 0

    for item in entries:
        dest_path = Path(item["dest"])
        src_path = Path(item["src"])

        try:
            if action == "move":
                if dest_path.exists():
                    src_path.parent.mkdir(parents=True, exist_ok=True)
                    shutil.move(str(dest_path), str(src_path))
                    restored_items += 1
            else:  # "copy"
                if dest_path.exists():
                    dest_path.unlink()
                    restored_items += 1
        except Exception:
            errors += 1

        for sc in item.get("sidecars", []):
            sc_dest = Path(sc["dest"])
            sc_src = Path(sc["src"])
            try:
                if action == "move":
                    if sc_dest.exists():
                        sc_src.parent.mkdir(parents=True, exist_ok=True)
                        shutil.move(str(sc_dest), str(sc_src))
                        restored_sidecars += 1
                else:  # "copy"
                    if sc_dest.exists():
                        sc_dest.unlink()
                        restored_sidecars += 1
            except Exception:
                errors += 1

    # Cleanup empty directories created during triage under Pictures, Videos, Pictures_Doc
    for sub in ('Pictures', 'Videos', 'Pictures_Doc'):
        target_sub = source_dir / sub
        if target_sub.exists() and target_sub.is_dir():
            for root, dirs, files in os.walk(target_sub, topdown=False):
                for d in dirs:
                    d_path = Path(root) / d
                    try:
                        d_path.rmdir()
                    except OSError:
                        pass
            try:
                target_sub.rmdir()
            except OSError:
                pass

    # Rename ledger to .bak to mark as rolled back
    bak_ledger = ledger_file.with_suffix('.json.bak')
    try:
        ledger_file.replace(bak_ledger)
    except Exception:
        pass

    return {
        "success": True,
        "restored_items": restored_items,
        "restored_sidecars": restored_sidecars,
        "errors": errors,
        "ledger_backup": str(bak_ledger)
    }


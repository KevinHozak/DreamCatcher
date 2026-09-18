"""
executor.py - Safe media mover and sidecar relocation engine for DreamCatcher
"""

import os
import shutil
import json
from pathlib import Path
from typing import Dict, List, Any, Tuple


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
        dest_file = target_dir / src_path.name

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
                    dest_sc = target_dir / sc.name
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

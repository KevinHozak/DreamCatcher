# 🕸️ DreamCatcher

DreamCatcher is a local-first Windows desktop application for triaging photo
archives and browsing indexed media. The desktop app uses **Tauri v2, React,
TypeScript, and Rust**. It can use a locally installed Ollama service for vision
classification; the Python/FastAPI backend remains available for development
and selected API workflows.

## What works today

- **Photo triage:** scan a source folder, classify obvious documents/photos,
  review ambiguous items in the Decision Deck, and organize approved photos
  into chronological clusters.
- **Safe file operations:** route output to configured picture/video folders,
  avoid overwriting collisions, preserve supported Takeout sidecars, and roll
  back completed triage using its ledger.
- **Media Library:** scan picture and video roots into a shared local SQLite
  inventory, browse/filter results, inspect metadata, and review duplicates
  in a strictly non-destructive review workflow.
- **Descriptions:** import sidecar and embedded descriptions, view provenance,
  and edit descriptions in both the native Tauri desktop app and Python backend.
  User edits survive rescans and never modify original media or sidecar files.
  Automated caption processing is not yet enabled with an approved local model.
- **People-search foundation:** settings, privacy boundaries, and derived face
  record storage exist. No face-recognition runtime is shipped, so indexing
  and actual face-based search are not ready.

## Run the desktop app

The launcher expects a previously built `DreamCatcher.exe` beside `start.bat`:

```powershell
.\start.bat
```

For development, install the [Ollama desktop runtime](https://ollama.com/),
start it, and pull the model used by the classifier:

```powershell
ollama pull moondream
```

Then run the Tauri development app:

```powershell
cd frontend
npm install
npm run desktop
```

The desktop app uses Ollama at `http://127.0.0.1:11434` for local vision
classification. The optional Gemini path is remote and should only be selected
when the user intends to send media for that processing.

## Python API development

The Python backend is a separate development/API path, not a dependency of the
packaged desktop launcher:

```powershell
cd backend
python -m venv .venv
.\.venv\Scripts\Activate.ps1
pip install -r requirements.txt
python main.py
```

It listens on `127.0.0.1:8080`. When using the browser frontend outside Tauri,
start Vite from `frontend` as well. The Python and native paths do not have
feature parity; see [the roadmap](implementation_plan.md).

## Local data and safety

- The canonical media inventory is SQLite at
  `%LOCALAPPDATA%\DreamCatcher\inventory.sqlite3` on Windows. The native app
  and Python backend share this location unless overridden for controlled
  deployments.
- People-search records, when created, live separately in
  `%LOCALAPPDATA%\DreamCatcher\people-index.sqlite3`.
- Inventory scans and indexing only record metadata. They do not change media
  files. Triage execution is a distinct user-approved operation with collision
  handling and rollback support.
- See [inventory storage](docs/inventory-storage.md), [caption behavior](docs/captions.md),
  [face-data privacy](docs/face-data-privacy.md), and
  [people-search feasibility](docs/people-search-feasibility.md).

## Project tracking

The current issue list and roadmap are maintained in GitHub:
[DreamCatcher issues](https://github.com/KevinHozak/DreamCatcher/issues) ·
[development project board](https://github.com/KevinHozak/DreamCatcher/projects).

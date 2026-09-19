# 🕸️ DreamCatcher

> Local AI Photo Triage Cockpit powered by **Moondream** (Ollama), **Tauri v2**, and **Rust**.

DreamCatcher eliminates the friction of organizing massive photo archives (like Google Photos Takeout). It pairs local computer vision with a focused, keyboard-driven triage deck to sift out utility screenshots, receipts, and labels from authentic family memories.

Built as a lightweight, native Windows desktop application with zero external Python or backend server requirements.

---

## 🌟 Core Features

- **Standalone Native Desktop App:** Bundled via Tauri v2 with a blazingly fast embedded Rust engine. Zero Python backend needed.
- **Local Vision AI:** Analyzes photos 100% offline using **Moondream** directly via local Ollama (`http://127.0.0.1:11434`).
- **The Clean Sweep:** Instantly batch-approves high-confidence receipts, paperwork, and screenshots.
- **The Decision Deck:** Fast, distraction-free keyboard triage (`D` for Document, `F` for Family Photo) for ambiguous items.
- **Event Clustering Studio:** Chronologically groups memories into holiday and event folders with auto-generated descriptions.
- **Sidecar Preservation:** Automatically pairs and relocates `.json` and `.supplemental-metadata.json` sidecars alongside each image and video.
- **Safe Execution & Rollback:** Collision avoidance naming (`_1`, `_2`) with full atomic rollback ledger support.

---

## 🚀 Quickstart

1. Ensure [Ollama](https://ollama.com/) is installed and running with `moondream`:
   ```powershell
   ollama pull moondream
   ```
2. Launch DreamCatcher:
   ```powershell
   .\start.bat
   ```
   *Alternatively, run from `frontend`:*
   ```powershell
   npm run desktop
   ```

---

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                 DreamCatcher.exe (Tauri)                    │
│                                                             │
│   ┌─────────────────────────────────────────────────────┐   │
│   │               React + Vite Frontend                 │   │
│   │   Decision Deck • Clean Sweep • Clustering Studio   │   │
│   └──────────────────────────┬──────────────────────────┘   │
│                              │ Tauri IPC (In-Process)       │
│   ┌──────────────────────────▼──────────────────────────┐   │
│   │              Rust Local Engine (src-tauri)          │   │
│   │   • scanner.rs    - EXIF, sidecars, filesystem scan │   │
│   │   • classifier.rs - heuristics, Ollama vision, cache│   │
│   │   • clustering.rs - events, holidays, geocoding     │   │
│   │   • executor.rs   - file/sidecar moves, rollback    │   │
│   │   • media.rs      - LRU thumbnail cache & protocol  │   │
│   └──────────────────────────┬──────────────────────────┘   │
└──────────────────────────────┼──────────────────────────────┘
                               │ HTTP (127.0.0.1:11434)
                               ▼
                 ┌───────────────────────────┐
                 │       Ollama Service      │
                 │   (moondream / gemma3)    │
                 └───────────────────────────┘
```

---

## 🗺️ Project Tracking

Concrete implementation tasks and phased epics are tracked on the [DreamCatcher Dev Project Board](https://github.com/KevinHozak/DreamCatcher/projects).

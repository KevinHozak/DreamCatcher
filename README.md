# 🕸️ DreamCatcher

> Local AI Photo Triage Cockpit powered by **Moondream** (Ollama) and FastAPI.

DreamCatcher eliminates the friction of organizing massive photo archives (like Google Photos Takeout). It pairs local computer vision with a focused, keyboard-driven triage deck to sift out utility screenshots, receipts, and labels from authentic family memories.

---

## 🌟 Core Features

- **Local Vision AI:** Analyzes photos 100% offline using **Moondream** via Ollama (with Gemini Flash-Lite cloud fallback).
- **The Clean Sweep:** Instantly batch-approves high-confidence receipts, paperwork, and screenshots.
- **The Decision Deck:** Fast, distraction-free keyboard triage (`D` for Document, `F` for Family Photo) for ambiguous or mixed items.
- **Event Clustering Studio:** Chronologically groups memories into holiday and event folders with auto-generated descriptions.
- **Sidecar Preservation:** Automatically pairs and relocates `.json` and `.supplemental-metadata.json` sidecars alongside each image and video.

---

## 🚀 Quickstart

1. Ensure [Ollama](https://ollama.com/) is installed and running with `moondream`:
   ```powershell
   ollama pull moondream
   ```
2. Run the application:
   ```powershell
   .\start.bat
   ```
3. Open `http://localhost:5173` in your browser.

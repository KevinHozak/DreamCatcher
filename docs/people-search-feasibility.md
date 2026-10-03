# Local People Search Feasibility Decision

**Epic Reference:** [#55](https://github.com/KevinHozak/DreamCatcher/issues/55) (Parent Epic)  
**Spike Decisions:** [#56](https://github.com/KevinHozak/DreamCatcher/issues/56) (Empirical Benchmarks), [#57](https://github.com/KevinHozak/DreamCatcher/issues/57) (Feasibility Contract & Verdict)  
**Status:** **Decision Gate Concluded — Explicit NO-GO for Bundled v1.0 Release; Opt-In Experimental Architectural Path Documented**

---

## 1. Final Decision & Verdict

**Verdict:** **NO-GO for Bundled Shipping in v1.0**; facial recognition indexing remains **officially deferred** from default desktop distribution.

DreamCatcher **will not ship** bundled face-detection or embedding model binaries in the Windows desktop installer (`.msi` / `.exe`), nor will it silently download model weights in the background.

The application preserves its strict privacy boundary, opt-in toggle, and human-in-the-loop review architecture. The UI and API will continue to report `state: "not_ready"` with clear explanations that face indexing is unavailable in the standard distribution.

---

## 2. Evaluation Findings Summary

### A. Performance & Runtime Feasibility (From Issue #56 Spike)
- **Runtime:** ONNX Runtime with `CPUExecutionProvider` and `DmlExecutionProvider` (DirectML GPU) successfully verified on Windows 11.
- **Inference Speed:**
  - SCRFD-0.5G (640x640 detection): **17.3 ms/photo (57.8 FPS)** on DirectML GPU; **30.0 ms/photo (33.4 FPS)** on 11th Gen Core i7 CPU.
  - MobileFaceNet (112x112 512-D embedding): **1.2 ms/face (838+ FPS)** on GPU; **0.97 ms/face (1030+ FPS)** on CPU.
- **Estimated Rebuild Duration (10,000 photos, ~25,000 faces):**
  - **DirectML GPU:** ~2.6–3.0 minutes total processing time.
  - **CPU Only:** ~5.5–6.5 minutes total processing time.
- **Memory Consumption:**
  - Peak observed RAM overhead during detection and feature embedding is **+315 MB** on CPU and **+448 MB** on DirectML GPU.
- **Network Isolation:**
  - Verified **0 network calls** across session creation, weight decoding, and inference execution (`zero_network_verified = true`).

### B. Storage Footprint & Database Sizing
- **Derived SQLite Database (`people-index.sqlite3`):**
  - Benchmarked with 10,000 photos and 25,000 face detections with 512-dimensional `float32` embeddings (2,048 bytes per face).
  - Vacuumed database size: **102.12 MB** total (~10.2 KB per photo).
  - Storage overhead is modest and fully independent of canonical `inventory.sqlite3`.

### C. Legal, Licensing & Redistribution Obligations
The evaluated candidate model weights present critical licensing and redistribution friction for a clean open-source desktop release:
1. **SCRFD (InsightFace):**
   - The official InsightFace model weights are released under **Non-Commercial Research Licenses** (InsightFace Non-Commercial License). Bundling these directly into DreamCatcher distribution binaries introduces significant legal exposure for general/commercial end users.
2. **Alternative Permissive Models (BlazeFace / YuNet / MobileFaceNet Apache-2.0):**
   - While runtime code (ONNX Runtime, OpenCV) is MIT/Apache-2.0, high-accuracy family album weights with unencumbered commercial redistribution rights require specialized curation, third-party hosting, or user-supplied checkpoints.
3. **Packaging Burden:**
   - Pre-bundling ONNX models and DirectML runtime binaries would inflate the installer package size by ~120–150 MB, which conflicts with DreamCatcher's lean initial desktop footprint.

---

## 3. Confirmed Safety & Privacy Contract

1. **Default State:**
   `people_search_enabled` defaults to `false`. No face database or derived table is created while disabled.
2. **Explicit Consent & Deferred Processing:**
   Enabling the toggle in Settings records user intent but **does not initiate processing**. It displays `state: "not_ready"` with an honest, non-deceptive status message.
3. **Strict Storage Boundary:**
   - Canonical inventory (`inventory.sqlite3`) and original media files are **immutable** with respect to face indexing.
   - All face detections, bounding boxes, clusters, and embeddings reside exclusively in `%LOCALAPPDATA%\DreamCatcher\people-index.sqlite3`.
4. **Independent Deletion Guarantee:**
   - The user can delete the derived people index at any time with one click.
   - Deletion removes `people-index.sqlite3`, `people-index.sqlite3-wal`, and `people-index.sqlite3-shm` without impacting photo inventory, captions, or file organization ledgers.
5. **Zero Remote Processing Guarantee:**
   - Facial recognition processing must **never** fall back to remote cloud vision APIs (such as Gemini or third-party web services). Remote face processing is strictly forbidden by policy.
6. **Human-in-the-Loop Review:**
   - Person identity queries match **only human-reviewed, confirmed labels**.
   - Raw model cluster candidates are never automatically assigned to identities or exposed as ground truth.
7. **Rebuild & Cancellation Contract:**
   - Any future background indexing job must support graceful cancellation, progress telemetry, and full index purge on demand.

---

## 4. Minimum & Recommended Hardware Specifications

For future implementation or advanced users configuring custom local face runtimes:

| Requirement | Minimum (CPU Fallback) | Recommended (DirectML GPU) |
| :--- | :--- | :--- |
| **Operating System** | Windows 10/11 64-bit | Windows 11 64-bit |
| **Processor** | 4-Core x86_64 CPU with AVX2 (Intel 8th Gen / AMD Ryzen 2000+) | 6-Core+ Modern CPU |
| **Graphics** | Integrated Graphics | DirectX 12 Feature Level 12_0 GPU (Intel Iris Xe, Radeon RX 6000+, NVIDIA GTX 1060+) |
| **RAM** | 8 GB RAM (>= 1 GB free process allocation) | 16 GB System RAM |
| **Disk Space** | 200 MB free (app + database) | 1 GB free (for 50,000+ photo collections) |
| **Throughput** | 25–35 photos/sec | 55–80+ photos/sec |

---

## 5. Future Implementation Gate (Conditions for Enabling)

Before any local face recognition indexing can transition from `not_ready` to `ready`:
1. **Model Weight Sourcing:** Adopt an explicitly permissive, commercially redistributable model pipeline (e.g. Apache 2.0 / MIT weights or an on-demand user-download flow with explicit license disclosure).
2. **Native Runtime Integration:** Expose native Tauri/Rust or packaged background worker bindings for ONNX Runtime DirectML.
3. **Interactive Review UI:** Deliver a native Decision Deck UI for face cluster clustering, verification, merging, and renaming.
4. **UI Honesty:** Maintain truthful status messages that never describe facial recognition as functional until fully backed by a verified local runtime.

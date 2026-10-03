# Exact duplicate scale benchmarks — issue #51

Measured on 2026-10-03 using the native Rust implementation at issue #51, based on main `cefdfd4`. Windows NT 10.0.26200.0; Intel64 family 6 model 140 stepping 2; 8 logical processors; rustc 1.96.0. CPU model name and installed RAM were unavailable through sandboxed CIM queries.

## Method

Run the ignored native test in separate processes for each size:

```powershell
$env:DREAMCATCHER_BENCH_FILES='5000' # repeat with '50000'
cargo test --manifest-path frontend/src-tauri/Cargo.toml duplicates::tests::benchmark_exact_duplicates -- --ignored --nocapture
```

Fixtures are created beneath a unique temporary directory and removed afterward. Each file is 4,096 deterministic bytes; consecutive pairs have identical content, and different pairs have different content. This yields 2,500 and 25,000 exact groups. A sidecar is added independently. Fixture creation is timed separately from a single SQLite WAL insertion transaction. This indexes synthetic inventory rows, not scanner/EXIF parsing.

Analysis invokes the same native function as desktop IPC: SQLite inventory read, SHA-256 hashing, grouping/sorting, and pretty JSON review-store persistence. Native duplicate groups are stored in `duplicates.json`, not a SQLite grouping transaction. Review-page timing includes reading/parsing the whole JSON store and returning at most 50 groups. Exclusion timing includes the full store rewrite. Export timing includes reading the store, constructing the complete review-only proposal, and JSON serialization; browser download and JavaScript serialization are not included.

A current-thread Tokio runtime samples a 16 ms heartbeat while analysis runs on `spawn_blocking`, the same dispatch mechanism as native IPC. The native implementation emits progress after each 100 files; the fixture verifies callback counts. Heartbeat gaps measure executor availability, not browser frame times or the actual Tauri event delivery path.

Peak working set is the Windows process high-water mark from `GetProcessMemoryInfo`, covering fixture creation, analysis, review, export, reanalysis, and integrity checks. It is not an incremental analysis-only allocation measurement. Both runs use the **debug test profile**, one sample per size, with OS caches warmed by fixture writes. These are reproducible scale checks, not release throughput claims. Small synthetic files stress file count and review group count; multi-megabyte photos, large videos, cold disks, network drives, and real image decoding are outside this fixture.

## Results

| Metric | 5,000 files | 50,000 files |
|---|---:|---:|
| Media payload | 19.53 MiB | 195.31 MiB |
| Exact groups | 2,500 | 25,000 |
| Fixture creation | 12.356 s | 143.035 s |
| SQLite inventory insertion transaction | 57.99 ms | 607.31 ms |
| Native analysis and review-store persistence | 2.693 s | 15.716 s |
| First review page | 84.00 ms | 529.77 ms |
| Exclusion toggle | 265.31 ms | 1,483.42 ms |
| Complete proposal serialization | 233.98 ms | 1,487.07 ms |
| Proposal bytes | 2,383,165 | 23,881,226 |
| Peak process working set | 35.89 MiB | 244.20 MiB |
| Executor ticks during analysis | 168 | 982 |
| Largest sampled executor gap | 32.14 ms | 32.18 ms |
| Original media hashes preserved | 5,000 / 5,000 | 50,000 / 50,000 |
| Sidecars preserved | 1 / 1 | 1 / 1 |

## Safety and review behavior

The test exercises analysis, paginated review, exclusion on/off, unknown-member rejection, full proposal export, and reanalysis. Exclusions survive reanalysis. Every original file is rehashed afterward, its digest compared with the baseline, and the media-directory entry count checked. The sidecar is compared byte-for-byte. No duplicate code opens media for writing or invokes a delete/move operation; the only persistent writes are derived review JSON. Proposal data carries `mode: review-only` and has no execution/deletion path.

Desktop analysis, review-store reads, exclusions, and proposal preparation now run on blocking workers rather than the IPC handler's foreground thread. A shared mutex serializes review-store operations, preventing an exclusion write from racing a reanalysis. Reads and edits may wait for an active analysis, while the UI thread stays available. UI controls disable exclusion/export during analysis. Group pages contain at most 50 groups; each group renders at most 50 members per member page. Full export includes all groups, beyond the first 100.

The normal 240-file regression verifies 120 groups are exported and originals remain unchanged. Interactive packaged desktop frame times and browser download behavior have not been measured. JSON store parsing/rewrites and full-proposal IPC serialization remain linear with archive size; these measurements should guide any later SQLite review-store migration rather than imply it already exists.

## Interpretation and follow-up

Both scale runs passed all integrity assertions, including exclusion retention and complete export. The 50,000-file analysis took 15.72 seconds while the executor continued sampling at roughly 16 ms, with a largest observed gap of 32.18 ms. This supports the background-dispatch design; it does not substitute for packaged UI frame-time verification.

The JSON review store remains the limiting review operation: at 50,000 files a first page took 530 ms, an exclusion rewrite 1.48 seconds, and complete proposal preparation/serialization 1.49 seconds. Worker dispatch prevents these operations from occupying the foreground IPC handler; pending exclusion controls avoid overlapping edits. A future SQLite-backed derived review store would reduce full-store reads and rewrites. Full export still constructs a roughly 22.8 MiB payload; JavaScript serialization/download performance needs a separate interactive measurement.

No media was automatically deleted or changed in either run. The exact matching scope is SHA-256 content equality only; perceptual/candidate clustering algorithms are not implemented or benchmarked here.

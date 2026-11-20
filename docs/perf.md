# Performance (P6-T14)

Measured on 2026-11-20 with `make perf`:
- an optimised build (release profile) with the test interface;
- run in the Ubuntu 26.10 dev container with a headless mutter;
- rendering on software OpenGL (Mesa llvmpipe), since the container has no
  GPU;
- the fixture home, plus a folder of 10,000 empty `.txt` files.

The raw numbers are in `artifacts/perf/app.json` and `artifacts/perf/E3-10.json`.

| Target (DESIGN.md §8.4) | Goal | Measured | Status |
|---|---|---|---|
| Launch to usable window, warm | under 0.5 s | 0.63 s (cold 1.4 s) | Missed |
| Open a folder of 10,000 items: first cards | under 150 ms | 1.0 s (listing done at 0.64 s) | Missed |
| Search as you type, 1 million entries | under 50 ms per keystroke | p95 27 ms, max 33 ms | **Met** |
| First full index of a 500 GB home | background, idle priority, app usable | Runs at nice 19 with idle I/O; not timed here | HUMAN |
| Memory, idle, one window | under 150 MB | 277 MB idle at the fixture home; 361 MB after the 10,000-item folder. With Qt's software renderer instead of llvmpipe: 112 MB | Unknown on real hardware |

## Fixed in this pass

- The list view's rows were rebuilt for every file in every view; now only
  in list view. The 10,000-item folder's first cards went from 1.4 s to 1.0 s.
- The file-type description ("PDF document") was looked up in the MIME
  database for every card; it is now looked up once per type.

## Misses, their causes, and the issues to file

The project has no issue tracker yet (HUMAN). These are the issues to open
when it has one.

1. **The first cards of a big folder wait for the whole listing.**
   - `FolderModel` reads every entry before showing any. It asks GIO for the
     full content type, two icons and the thumbnail state of each file; the
     listing alone takes 0.64 s for 10,000 files.
   - After that, building and parsing the grid's JSON for all rows takes
     about 0.35 s.
   - The fix:
     - publish the first batch of 500 entries as soon as it arrives;
     - ask only for `fast-content-type` while listing, and fetch icons and
       thumbnail state for the cards on screen;
     - send rows to QML in pages.
2. **Warm launch is 0.13 s over the target.**
   - QML is compiled when the app starts.
   - The fix: compile it ahead of time (qmlcachegen through cxx-qt-build's
     QML module support), and start the first folder listing while the
     window is still being created.
3. **Memory.**
   - In the container, about 165 MB is Mesa's software OpenGL: compare
     277 MB with 112 MB under Qt's software renderer.
   - A real GPU driver behaves differently, so the target has to be measured
     on real hardware (HUMAN).
   - If it still misses there, look at the QML component cache and the
     thumbnail cache.

## How to repeat

```bash
make perf          # builds, runs E3-10 in release, then tests/tools/perf.py
```

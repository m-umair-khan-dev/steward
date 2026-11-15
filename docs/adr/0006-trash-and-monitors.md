# ADR 0006: Trash and file monitors in the test session

- **Status:** accepted
- **Date:** 2026-11-15
- **Task:** P0-S6
- **Design reference:** DESIGN.md §10.2, Phase 2 and Phase 1 (live refresh)

## Context

Phase 2 tests trash and restore files; Phase 1 tests live refresh. Both run
against a per-test home on the container's tmpfs. GIO's trash needs the file
and the home Trash on the same filesystem; file monitors need inotify. The named
fallback was an ext4 loop volume for test homes.

## Findings

Probe: `tests/spikes/probe.py` (S6), on a tmpfs home in the e2e container.

- `gio trash` on a file in the home: exit 0; the file arrives in
  `~/.local/share/Trash/files/`.
- A GIO directory monitor reports `created`, `changed` and
  `changes-done-hint` for a new file within 100 ms.

## Decision

Test homes stay on tmpfs. The ext4 loop-volume fallback is not needed.

## Consequences

- Trash tests must keep files inside the test home. Files on another filesystem
  (for example a second mounted volume in Phase 4) have no home-Trash path;
  those tests need a `.Trash-$UID` folder on that volume, or should expect GIO's
  "cannot trash" error.

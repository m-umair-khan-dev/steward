# Progress

Read this first in every session (AGENTS.md rule 2). Update it before stopping.

## Current phase

**Phase 1: Browse (a read-only file manager)**, in progress.

Phase 0 is complete; its gate passed on Ubuntu 26.10 on 2026-11-15. The product
owner asked to start Phase 1 while Phase 0's 26.10 run was finishing (an
exception to AGENTS.md rule 1, approved by them).

## Release targets

Develop and gate on **Ubuntu 26.10** (ADR 0007). 24.04 and 26.04 are tested once
the whole project is complete. Code stays within the 24.04 APIs (ADR 0001).

## Phase 0: Foundations, harness and spikes (complete)

| ID | Task | Status | Notes |
|---|---|---|---|
| P0-T01 | Repository skeleton | done | |
| P0-T02 | Cargo workspace | done | |
| P0-T03 | Docker images, Compose, Makefile | done | Images for 24.04, 26.04, 26.10 |
| P0-T04 | Hello window | done | |
| P0-T05 | Fixture home and guard | done | 13 generator tests |
| P0-T06 | Headless runner and end-to-end harness | done | Real key presses work (ADR 0003) |
| P0-T07 | CI workflow | done | HUMAN: enable on the Git host |
| P0-S1 | Spike: library versions | done | ADR 0001 |
| P0-S2 | Spike: headless compositor | done | ADR 0002: mutter default, weston fallback |
| P0-S3 | Spike: accessibility tree and input | done | ADR 0003: both work |
| P0-S4 | Spike: GUI from the container | done | ADR 0004: Broadway on Docker Desktop |
| P0-S5 | Spike: bwrap in containers | done | ADR 0005: works without a fresh /proc |
| P0-S6 | Spike: Trash and monitors | done | ADR 0006: work on tmpfs |

Gate results on 26.10: lint ok; 6 Rust and 13 fixture tests pass; end-to-end
E0-01 to E0-05 plus the key-press harness test pass. The same also passed on
24.04 before ADR 0007.

Scenario fix recorded under rule 7: E0-02 first targeted a "Close" button by
role "button" (AT-SPI calls it "push button" on 24.04) and without a scope (the
window has its own "Close"). It now targets `within="About"`, and the helper
treats role names that changed between releases as the same.

## Phase 1: Browse (complete on 26.10, pending HUMAN sign-off)

Gate on Ubuntu 26.10, 2026-11-19: `make check` green. Lint ok; 35 Rust tests
and 13 fixture tests pass; end-to-end E0-01 to E0-05, the key-press harness
test and E1-01 to E1-18 pass.

| ID | Task | Status | Notes |
|---|---|---|---|
| P1-T01 | Folder listing in core | done | `FolderModel` (L2) |
| P1-T02 | Kind table | done | Adds an Audio kind |
| P1-T03 | Grid and list views | done | E1-01, E1-02, E1-08 |
| P1-T04 | Navigation | done | E1-03 |
| P1-T05 | Path bar | done | E1-04 (real keys, Tab completion) |
| P1-T06 | app.db, sidebar | done | E1-10 |
| P1-T07 | Grouping, summary, chips, status | done | E1-01, E1-05 |
| P1-T08 | Thumbnails | done | E1-09 |
| P1-T09 | Preview pane | done | E1-03 |
| P1-T10 | Live refresh, hidden files, sort, per-folder memory | done | E1-02, E1-06, E1-07 |
| P1-T11 | Open and Open With | done | E1-12 |
| P1-T12 | Settings and Preferences | done | E1-02, E1-13 |
| P1-T13 | Empty, denied, gone, loading states | done | E1-11 |
| P1-T14 | Menus, shortcuts window, About | done | E1-14, E0-02 |
| P1-T15 | Qt/QML front end (ADR 0009) | done | All earlier scenarios pass on the Qt build |
| P1-T16 | Reference look (ADR 0008) | done, HUMAN sign-off | Review `artifacts/screens/` against `fileosmac/screenshots/smart-downloads.png` |
| P1-T17 | "Filter this folder" | done | E1-16 |
| P1-T18 | Collapsible sidebar and rail | done | E1-17; hover slide-out is on the HUMAN list |
| P1-T19 | Motion | done | E1-18 |

T15 to T19 share one commit: they were built as one QML tree and do not split
into working pieces.

Scenario fixes recorded under rule 7 (toolkit switch, ADR 0009):
- E0-02 expected the dialog type "AdwAboutDialog"; the Qt build reports "about".
- E1-14 expected GTK accelerator names (`<Control>l`); the table now uses Qt
  key-sequence syntax (`Ctrl+L`). The shortcuts themselves are unchanged.

Bugs found and fixed while bringing up the Qt build:
- The test state could say "ready" one main-loop turn before the controller
  rebuilt its item list, so `Select` right after loading found nothing (E1-03).
  The controller now reports the load state as of its own last rebuild.
- A grid rebuild could cancel the thumbnails the new cards had just asked for:
  Qt destroys old cards after creating new ones. Cards now report on-screen by
  file URI with a count (E1-15 caught it).
- Ctrl- and Shift-click read modifiers from the wrong object and never worked.

Review screenshots: `docker compose -f compose.desktop.yaml run --rm e2e
docker/scripts/run-e2e python3 tests/tools/capture_screens.py` writes
`artifacts/screens/`.

## Blocked / waiting for a human

- Enable CI once the repository is on its Git host (P0-T07).
- With Docker Engine (not Docker Desktop), run `make app` once and confirm the
  window opens on the desktop (ADR 0004).
- Confirm or replace the codename "Cairn" and app ID `dev.cairn.Files`.
- P1-T16: sign off the look against the reference captures.
- E1-17: check that pointing at the rail slides the sidebar out (pointer input
  is not automated).
- Decide whether to add tabs (the reference shows a second header row of tabs).
- Docker Desktop on this machine needs the user in the `kvm` group (done on
  2026-11-19); without it Docker Desktop stops at start-up after a reboot.

## Next

- Phase 2 waits for the product owner's go-ahead.

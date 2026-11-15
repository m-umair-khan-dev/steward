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

## Phase 1: Browse

| ID | Task | Status | Notes |
|---|---|---|---|
| P1-T01 | Folder listing in core | done (L2) | `FolderModel`, tested against the fixture |
| P1-T02 | Kind table | done (L1, L2) | Agrees with the fixture generator. Adds an Audio kind |
| P1-T03 | Grid and list views | in progress | |
| P1-T04 | Navigation | todo | |
| P1-T05 | Path bar | todo | |
| P1-T06 | app.db, sidebar | in progress | Store and migrations done (L1) |
| P1-T07 | Grouping, summary, chips, status | in progress | Rules in core done (L1) |
| P1-T08 | Thumbnails | in progress | |
| P1-T09 | Preview pane | todo | |
| P1-T10 | Live refresh, hidden files, sort, per-folder memory | in progress | Live refresh done (L2) |
| P1-T11 | Open and Open With | todo | |
| P1-T12 | Settings and Preferences | in progress | Schema done |
| P1-T13 | Empty, denied, gone, loading states | in progress | |
| P1-T14 | Menus, shortcuts window, About | todo | |

## Blocked / waiting for a human

- Enable CI once the repository is on its Git host (P0-T07).
- With Docker Engine (not Docker Desktop), run `make app` once and confirm the
  window opens on the desktop (ADR 0004).
- Confirm or replace the codename "Cairn" and app ID `dev.cairn.Files`.

## Next

- Finish the Phase 1 UI and its end-to-end scenarios E1-01 to E1-15.

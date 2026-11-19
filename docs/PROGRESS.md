# Progress

Read this first in every session (AGENTS.md rule 2). Update it before stopping.

## Current phase

**Phase 3: Index, folder sizes, search and Peek**, complete on 26.10 (gate
green 2026-11-20), pending the HUMAN check on a large real home. Phase 4
waits for the product owner's go-ahead.

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

## Phase 2: File operations and Undo (complete on 26.10)

Gate on Ubuntu 26.10, 2026-11-19: `make check` green. Lint ok (with the E2-13
ops check); 64 Rust tests (22 engine L2 tests) and 13 fixture tests pass;
end-to-end E0-01 to E2-14 (38 scenarios) pass.

| ID | Task | Status | Notes |
|---|---|---|---|
| P2-T01 | `core::ops`: operations, batches, queue, progress, cancel | done | L2 tests per operation |
| P2-T02 | Journal, undo and redo, never overwrite, partial batches | done | L2 tests for every undo edge case |
| P2-T03 | New folder, rename in place, Move to Trash, duplicate | done | E2-01, E2-02 |
| P2-T04 | Clipboard: text/uri-list and gnome-copied-files | done | E2-03, E2-04, E2-08 (verified headless with wl-paste/wl-copy) |
| P2-T05 | Drag and drop | done, HUMAN | L2 for the decision; the gesture is on the HUMAN list |
| P2-T06 | Clash dialog | done | E2-05 |
| P2-T07 | Queue UI with progress and Cancel | done | E2-07 |
| P2-T08 | Toasts with Undo; Ctrl+Z, Ctrl+Shift+Z; menu names the undo | done | E2-01 to E2-04 |
| P2-T09 | Trash view: list, Put Back, Empty Trash asks | done | E2-09 |
| P2-T10 | Context menus | done | Items, selection, background, Trash |
| P2-T11 | Failure handling | done | E2-10; disk full reported (seen in E2-07's first run) |
| P2-T12 | Crash safety | done | E2-11, E2-12 |
| P2-T13 | Static ops check in `make lint` | done | E2-13 |

Decisions:
- ADR 0010: the Trash is implemented directly (FreeDesktop spec) instead of
  `g_file_trash`, so Undo knows each item's exact place.
- Copies use our own block loop (not `g_file_copy`): precise progress and
  cancel, and holes stay holes. GIO's copy filled the test tmpfs with a 2 GB
  sparse file.
- Undo of a copy sends the copy to the Trash rather than deleting it, so Undo
  itself never loses data.
- Test hook `CAIRN_OPS_STEP_DELAY_MS` (e2e builds only) slows each step so a
  test can cancel or kill midway (E2-07, E2-11).

Harness fix (rule 7, not a weaker test): the compositor could drop the very
first key press after launch; the harness now sends a lone Shift once the
window is active, before the test's keys.

UI follow-up from the product owner (2026-11-19): the sidebar was restyled
closer to the reference (larger app row, headings and rows, lighter panel,
collapsible "Locations ›"). The reference's Workspaces, Bookmarks and Tools
sections arrive with Phases 4, 5 and 7.

## Phase 3: Index, folder sizes, search and Peek (complete on 26.10)

Gate on Ubuntu 26.10, 2026-11-20: `make check` green. Lint ok; 74 Rust tests
and 13 fixture tests pass; end-to-end E0-01 to E3-12 (49 scenarios) pass.
E3-10: one million entries, p95 32 to 36 ms against a 50 ms limit
(`artifacts/perf/E3-10.json`).

| ID | Task | Status | Notes |
|---|---|---|---|
| P3-T01 | `cairn-indexer`: crawl at idle priority, batched writes, exclusions | done | L2 count equals the manifest |
| P3-T02 | `index.db` with a trigram name index | done | L2 |
| P3-T03 | Size and count roll-up, incremental | done | L2, E3-02 |
| P3-T04 | Watching (GIO monitors, inotify); timed re-crawls past the limit; Notify from the app | done | E3-05, E3-06 |
| P3-T05 | D-Bus control; systemd user unit and activation file | done | E3-11, E3-12; unit and service file in `data/` |
| P3-T06 | Folder cards with count and size; two-level outline | done | E3-02 |
| P3-T07 | Search overlay on Ctrl+K | done | E3-03 |
| P3-T08 | Ranking; a new keystroke supersedes the last query | done | L1, E3-10 |
| P3-T09 | Result keys | done | E3-04 |
| P3-T10 | Folder names in the path bar | done | E3-09 |
| P3-T11 | Peek: images, PDF, text and code, audio and video, Info | done | E3-08 |
| P3-T12 | Preferences › Search and index | done | E3-11 |

Scenario adjustment (rule 7): E3-01 and E3-07 compare the index with the
manifest plus `.config/dconf/user`, the settings database that the session
writes into the home as soon as a setting changes. The manifest only knows
the planted files; that database is real user data and the index is right
to count it. Two harness leaks were fixed instead of adjusting the test:
Mesa's shader cache (written by the app) and mutter's session state no
longer land in the test home.

Bugs found while bringing it up:
- The indexer's Pause handler re-entered its own state through the
  settings "changed" signal; settings reactions now wait a main-loop turn.
- Search first read full rows for up to 10,000 candidates (p95 53 ms); it
  now ranks on names and reads full rows only for the results shown.
- The E3-10 timing test starved a file-monitor test when run in parallel;
  nextest now runs it alone (`.config/nextest.toml`).

## Blocked / waiting for a human

- Enable CI once the repository is on its Git host (P0-T07).
- With Docker Engine (not Docker Desktop), run `make app` once and confirm the
  window opens on the desktop (ADR 0004).
- Confirm or replace the codename "Cairn" and app ID `dev.cairn.Files`.
- P1-T16: sign off the look against the reference captures.
- E1-17: check that pointing at the rail slides the sidebar out (pointer input
  is not automated).
- Decide whether to add tabs (the reference shows a second header row of tabs).
- P2-T05: drag files between Cairn and Files (Nautilus) both ways; Ctrl-drag
  copies; copy in one and paste in the other.
- Phase 3: index a real, large home; watch CPU, disk activity and battery
  behaviour (the indexer runs at nice 19; the systemd unit adds idle I/O).
- Docker Desktop on this machine needs the user in the `kvm` group (done on
  2026-11-19); without it Docker Desktop stops at start-up after a reboot.

## Next

- Phase 4 (Smart Downloads, Dual pane, Workspaces) waits for the product
  owner's go-ahead.

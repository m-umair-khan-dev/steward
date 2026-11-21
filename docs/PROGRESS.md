# Progress

Read this first in every session (AGENTS.md rule 2). Update it before stopping.

## Current phase

**Phase 7: Developer and content tools**, in progress (started 2026-11-20).
Phase 6 is complete on 26.10 except the website (P6-T10), which is on hold.

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

## Phase 4: Smart Downloads, Dual pane and Workspaces (complete on 26.10)

Gate on Ubuntu 26.10, 2026-11-20: `make check` green. Lint ok; 82 Rust tests
and 13 fixture tests pass; end-to-end E0-01 to E4-14 (64 scenarios) pass.

| ID | Task | Status | Notes |
|---|---|---|---|
| P4-T01 | `core::plan`, fake clock `CAIRN_FAKE_NOW` | done | L1 |
| P4-T02 | Tidy planner | done | L1 against the manifest |
| P4-T03 | Tidy plan dialog and run; one Undo; Ctrl+Shift+O | done | E4-01 to E4-05 |
| P4-T04 | `core::dupes` with a hash cache | done | L2 with planted duplicates |
| P4-T05 | Downloads panel: space by kind, suggestions (`PackageDb`) | done | E4-06, E4-07 |
| P4-T06 | Preference: which folder is Downloads | done | E4-08 |
| P4-T07 | Dual pane | done | E4-09, E4-10 |
| P4-T08 | Workspaces: model, sidebar, page, add by menu or drop | done | E4-11 |
| P4-T09 | Workspace resilience: inode, Locate…, unplugged, Alt+1–9 | done | E4-12, E4-13 |
| P4-T10 | Folder symbols and colours, attributes with a database fallback | done | E4-14 |

Decisions and findings:
- Index schema 2 (rebuilt by itself): a size index for duplicate candidates
  and the filesystem and inode of each entry for re-finding workspace items.
- Folder-symbol rules run in this order: chosen, special folders, projects
  (.git or a build file), name words, majority kind. Name words come before
  the majority kind because E4-14 wants "Invoices" (three PDFs) to show the
  receipt symbol and a mostly-PDF folder the PDF symbol.
- The fixture plants no Git repository; E4-14 creates `.git` in "School" so
  the rule is shown to work on contents, not names.
- E4-11's second volume is `/dev/shm`, a separate tmpfs in the container.
- Test environment: the e2e container's `/tmp` now allows running programs
  (Docker's tmpfs default is noexec), so tests can put stand-ins such as
  `dpkg-query` on PATH as DESIGN.md §10.2 intends.

Bugs found and fixed:
- Return in the rename field also reached the folder view (Qt's TextInput
  passes Return on after "accepted") and opened the item just renamed. Seen
  as "No app could open notes.txt" in E2-01 since Phase 2; E2-01 now checks.
- The workspace banner rendered dark grey: the colour helper did not accept
  colour names.

## Phase 5: Smart Clean, Installed and About This Computer (complete on 26.10)

Gate on Ubuntu 26.10, 2026-11-20: `make check` green. Lint ok; 107 Rust tests
and 13 fixture tests pass; end-to-end E0-01 to E5-11 (75 scenarios) pass.

| ID | Task | Status | Notes |
|---|---|---|---|
| P5-S1 | Spike: last-launched date; system sizes without root | done | ADR 0011: no trustworthy date, "Unused apps" dropped |
| P5-T01 | `Suggester` trait, registry, scan that reports each card | done | L2 `crates/core/tests/clean.rs` |
| P5-T02 | Disk overview: free space, parts, the ring | done | E5-01 |
| P5-T03 | Eight suggesters; allow-list in `data/clean/dev-caches.toml` | done | L2 per card against the manifest; E5-01 to E5-04 |
| P5-T04 | Overview: two groups with totals, Looking…, Look Again, headline | done | E5-01 |
| P5-T05 | Review page: reasons, ticks, Tick all, Keep, Peek, Show in folder, sticky bar | done | E5-04 |
| P5-T06 | Move to Trash as one batch, sized toast, Undo; Trash and Empty Trash on the page | done | E5-05, E5-06 |
| P5-T07 | "Needs administrator": sizes and commands to copy, nothing run | done | E5-07 |
| P5-T08 | `core::packages`: ten sources, parsers on recorded output | done | L1 `crates/core/tests/packages.rs` |
| P5-T09 | Installed view: tool chips, filters, why, sizes, Save List, remove command | done | E5-08, E5-09 |
| P5-T10 | `core::sysinfo`: `SysProbe` with real and fixture probes | done | L1 with `tests/data/sysinfo/fixture.json` |
| P5-T11 | About This Computer: Hide Names, Copy as Text, from the computer row | done | E5-10; E5-11 read-only proof |

Decisions and findings:
- Smart Clean has eight cards (ADR 0011). Each path belongs to the first card
  that finds it (cards are in the overview's order), so no size is counted
  twice. Big files are files of 256 MB or more outside hidden folders.
- The disk bar shows "Your files", "Apps" (dpkg's installed sizes, Snap
  images, system Flatpaks; only when /usr is on the home disk), "In the
  Trash" and "System and other". DESIGN.md §4.3 lists System and Other
  separately; telling them apart needs root, so they are one part.
- Battery facts come from `/sys/class/power_supply` (readable by everyone)
  rather than UPower; the numbers are the same, and the cycle count is only
  there anyway.
- Installed asks each tool read-only, and makes sure asking writes nothing:
  - Cargo runs only when its install records exist, because
    `cargo install --list` creates them otherwise.
  - npm runs with logs and the update check off.
  - pip runs without its version check.
  - Homebrew is told not to update itself.
  - pipx is read from each venv's `pipx_metadata.json`, because running pipx
    logs the run.
- Hashing for duplicates opens files with `O_NOATIME`: reading a file to
  compare it is not opening it, and "not opened for 90 days" must stay true.
- The Trash folders are made when something is first trashed or the Trash is
  opened, no longer at start-up: starting the app and only reading must
  change nothing (E5-11).
- Tool pages (Free Up Space, Installed, About) cover the folder like a
  workspace page. The header shows the tool's name, the folder controls and
  path bar hide, and Back returns to the folder.
- Fixture recipe 2 adds the following, plus every card's expected items in
  the manifest:
  - folders called `cache` that are not on the allow-list (E5-03);
  - a symlink to Documents inside the npm cache (E5-06);
  - Snap and Flatpak leftovers, one of them still installed;
  - two more duplicate sets.

Test harness fixes:
- Each test's home is a `cp -a` copy of the fixture. Copying reads every
  file, which moves the source's access times to now, and `cp -a` carries
  those over. So the planted "not opened for 200 days" never reached a test.
  The harness now records the planted times when the fixture is built and
  puts them back on every copy.
- `tests/e2e/harness/fakes.py`: stand-ins for system tools that log every
  call and answer with recorded output (longest matching arguments win).
  E5-07 checks from that log that nothing was run as administrator.
- E2-08's docstring named an ADR 0011 about the clipboard that was never
  written. The reference is removed; the test still skips when the
  compositor offers no clipboard to other clients.
- Qt's shader cache (`~/.cache/qtshadercache-*`) counts as the app's own
  data in read-only checks.

## Phase 6: Release 1.0 (complete on 26.10, website on hold)

| ID | Task | Status | Notes |
|---|---|---|---|
| P6-T01 | Welcome on first run: demo, tour, make it yours | done | E6-01 |
| P6-T02 | What's New once after an update, from the release notes | done | E6-02 |
| P6-T03 | Preferences: General, Appearance, Shortcuts, Search and Index | done | Preferences pages test; E6-04 |
| P6-T04 | Translations: gettext everywhere, drafts in five languages, pseudo-locale | done | E6-03 |
| P6-T05 | Accessibility pass | done | E6-04 |
| P6-T06 | Desktop file with actions, AppStream, placeholder icons | done | E6-07 (metadata); desktop-actions test |
| P6-T07 | Optional default file manager: FileManager1, folder handler | done | E6-09 |
| P6-T08 | Packaging: cargo-deb, user unit for the indexer, lintian | done | E6-05 (`make deb-test`: apt install in a clean Ubuntu, 6 smoke scenarios); lintian no errors or warnings |
| P6-T09 | apt repository tooling: aptrepo image, publish-apt, dev key, apt-install-e2e | done | E6-06 (`make apt-e2e`: install 1.0.0-1, upgrade to 1.0.0.1-1) |
| P6-T10 | Website | ON HOLD | Paused by the product owner on 2026-11-20; draft in site/ (see site/README.md). E6-08 waits with it |
| P6-T11 | Screenshot generator (`make shots`) | done | 8 screens, light and dark, in artifacts/shots/ |
| P6-T12 | Release workflow; compose.yaml, compose.test.yaml, compose.prod.yaml | done (dry run HUMAN) | `.github/workflows/release.yml`; a -dry tag publishes to ci. The local equivalent (`make deb deb-test apt-e2e`) passes |
| P6-T13 | Report a Bug; SECURITY.md; code of conduct | done | Report-a-bug test |
| P6-T14 | Performance pass, docs/perf.md | done | Search met; launch, big folder and memory missed, explained with issues to file (docs/perf.md) |

E6-10 (the full regression on 24.04 and 26.04) is the final task after
Phase 9 (ADR 0007), not part of this phase's gate.

Decisions and findings:
- Version 1.0.0. Release notes live in site/src/content/changelog/<version>.md
  and feed What's New, the Debian changelog and (later) the site; a version
  without notes does not build.
- Test builds skip the welcome unless CAIRN_NO_WELCOME is set empty (E6-01
  does); release builds always show it on a first run.
- QML strings go through `p.t()` (gettext on the presenter); the five
  catalogs are AI drafts, marked so in their headers (HUMAN review).
  Some messages are sentence fragments joined in code; whole sentences with
  placeholders are a later improvement.
- Cairn has one window, so the desktop file's "New Window" brings the window
  forward at the Home folder. A second launch hands its request to the
  running Cairn over org.gtk.Actions.
- FileManager1 is served only while Cairn runs and Files does not own the
  name; no D-Bus activation file, because it would clash with Nautilus's.
- Smoke scenarios (tests/e2e/test_smoke.py) use only public interfaces so
  they run against the installed release build, which has no Test1. E0-01
  and E1-01 lost the smoke mark: they need Test1.
- Lintian runs in the package stage; the clean install-test image has only
  what a user's machine would.
- Docker's Ubuntu image skips translations; the install-test image re-enables
  them so the test sees what a desktop gets.
- Performance: two fixes; three targets missed and explained in docs/perf.md
  (measured on software OpenGL in a container).
- Scenario change (rule 7): E1-14 accepts a shortcut row named
  "Back: Alt+Left": since P6-T05 each row's accessible name carries its keys,
  so screen readers hear them.
- Bug found by the gate under load: reloading the folder on screen briefly
  emptied the list and dropped the selection (E1-03). The selection is now
  pruned only once a listing is complete.
- Scenario change (rule 7): E3-11 opens the Search and Index page before
  clicking Pause indexing, because P6-T03 moved the index settings there.

## Phase 7: Developer and content tools (in progress)

| ID | Task | Status | Notes |
|---|---|---|---|
| P7-S1 | Spike: Git library | done | gix 0.90, read-only (status, sha1) |
| P7-T01 | Git chip | done | E7-01, E7-02 |
| P7-T02 | Open In | done | E7-03 |
| P7-T03 | Path-bar commands | done | E7-03 |
| P7-T04 | core::agents | done | ADR 0013; E7-04 |
| P7-T05 | Local servers with Stop | done | E7-05 |
| P7-T06 | AI Agents view | done | E7-04, E7-05 |
| P7-T07 | Markdown view | done | E7-06. Remote images (inline, reference and `<img>`) are removed before QML renders; local images only from the document's folder |
| P7-T08 | Bookmarks | done | E7-07. `ureq` (rustls) fetches title and picture, bounded in time and size; the Preferences switch turns it off. Four views; paste, drop, Add Link (Ctrl+Shift+L) |
| P7-T09 | Content search | done | E7-08, E7-09. The indexer reads text in `bwrap` (ADR 0005 profile) by running itself as `cairn-indexer extract FILE`; 20 s limit; Home only, plus the `content-folders` setting (no Preferences UI for it yet). `poppler-utils` added to the image and to Recommends; `bubblewrap` added to Depends |
| P7-T10 | `cairn` command-line tool | done | E7-10. Commands live in `cairn_core::cli`; `cairn` forwards them to the running window (or runs them at start-up); `cairn-cli` does the same without Qt and starts `cairn` when none runs (not packaged). Desktop action "Add Link". Man page updated |
| P7-T11 | Customisable shortcuts | done | E7-11. Changes are kept in the `shortcuts` setting (a{sas}) over the built-in table; Preferences › Shortcuts records a key (app shortcuts pause meanwhile), clears one, restores defaults, and refuses a key already in use with a message. A filter field finds a shortcut |

Notes:
- Scenario change (rule 7), E7-06: the check that "a remote image causes no
  request" looks at image syntax in the preview text, because a reference
  definition line (`[r]: http://…`) stays in the text as a link target and
  loads nothing; the test server's request log is the real proof.
- Scenario change (rule 7), E1-18: the scenario now takes up to four frames
  during the flight and needs one that differs from both the start and the
  end, instead of a single frame. One screenshot takes about a fifth of the
  0.7 s flight, so on a loaded machine the single frame sometimes landed at
  take-off (differences 0.56 to 0.97 against the 1.0 threshold). What it
  checks is unchanged: motion over several frames, not a jump.
- Fix found by the gate: when the index changed while search was open, the
  query ran again and the selection jumped back to the first result, so
  Enter could open a different file than the one chosen (E3-04 failed under
  load). A re-run now keeps the chosen result.
- 2026-11-21: the product owner deleted all Docker images and volumes to free
  disk space; the dev image and fixture were rebuilt.

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
- Phase 4: Tidy a copy of a real, messy Downloads folder and judge the
  result. Check a workspace item on a real external drive after unplugging
  and replugging (shown greyed with the drive's name while unplugged).
- Phase 5: on a real Ubuntu install, compare Smart Clean's sizes with `du`,
  compare About with Settings › System › About, and read every suggestion's
  reason for clarity. Check the sidebar's Installed count against
  `apt list --installed`, `snap list` and `flatpak list`.
- Phase 7: try Open In with the terminals and editors actually installed;
  check the AI Agents numbers against a real machine's usage.
- Phase 6 (from DESIGN.md's HUMAN list):
  - final name and icon (the icon is a placeholder);
  - review the five translations, or mark them beta;
  - create the project signing key and store it as APT_SIGNING_KEY in CI;
  - choose hosting and DNS; set the repository URL in Cargo.toml and the
    placeholder addresses (maintainer, security, conduct, metainfo, the
    download page);
  - enable Actions and push a -dry tag to see the release workflow publish
    to ci;
  - install from staging on real 24.04 and 26.04 machines and use it for a
    day;
  - one pass with the Orca screen reader;
  - measure launch and memory on real hardware (docs/perf.md);
  - file the performance issues listed in docs/perf.md;
  - tag the release.
- Website (P6-T10) is on hold by the product owner's decision; E6-08 waits
  with it. The draft is in site/.
- Docker Desktop on this machine needs the user in the `kvm` group (done on
  2026-11-19); without it Docker Desktop stops at start-up after a reboot.

## Next

- P7-T11, then the Phase 7 gate
  and report. The website (P6-T10) resumes when the product owner says so.
- Known gaps to look at in Phase 6:
  - The collapsed sidebar rail does not list the tool pages yet.
  - Smart Clean walks the home folder instead of using the index (fine on
    the fixture; to measure in the performance pass, P6-T14).
  - Page strings in QML are not yet translatable (P6-T04).

# ADR 0014: Desktop integration on the fallbacks until P8-S1 is run

- **Status:** proposed (to confirm on real 24.04 and 26.04 desktops)
- **Date:** 2026-11-22
- **Task:** P8-S1, P8-T02, P8-T03, P8-T05
- **Design reference:** DESIGN.md §4.4, §7.3, Phase 8

## Context

Spike P8-S1 asks four questions that only a real GNOME desktop can answer:
global shortcut through the GlobalShortcuts portal or a GNOME custom
keybinding; how the tray icon behaves; whether a Shell extension can place
the Shelf top-centre and keep it above; whether it can see a file drag
begin. The product owner will check these on real desktops later and asked
for the work to continue meanwhile. DESIGN.md names a fallback for each.

## Findings

Seen in the 26.10 dev container (no human, no real desktop):

- gnome-settings-daemon's `media-keys` schema with relocatable
  `custom-keybinding` entries is installed (package
  gnome-settings-daemon-common); writing a keybinding there works from the
  app and is visible to `gsettings` in the same session.
- `org.gnome.settings-daemon.global-shortcuts` is also installed on 26.10,
  so the portal route probably exists there; whether it does on 24.04, and
  how its consent dialog behaves, is what the spike must establish.
- There is no panel in the test session; the tray can only be tested with a
  stand-in StatusNotifierWatcher (E8-06).

## Decision

Until P8-S1 is run:

- **Shelf key:** a GNOME custom keybinding running `steward shelf`, written
  only while "Open the Shelf with a key" is on (off by default) and removed
  when it is turned off. Default key Ctrl+Alt+Space. This route works on
  both 24.04 and 26.04. The portal route is added only if the spike shows a
  better experience.
- **Tray icon:** StatusNotifierItem. On a desktop without a watcher (stock
  GNOME without the AppIndicator extension) there is no icon; Ubuntu ships
  the extension. Off by default.
- **Shelf placement:** wherever the compositor puts it.
- **Open on drag:** not done.
- **Shell extension (P8-T05):** not started; it waits for the spike and for
  open decision O4.

## Consequences

- HUMAN checks in docs/PROGRESS.md: the key from another app, the tray icon
  on the real panel, Shelf placement, and the portal route on 26.04.
- If the portal route is chosen later, P8-T02 changes inside
  `crates/app/src/desktop.rs` only; the setting and the Preferences page
  stay.

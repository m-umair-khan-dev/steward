# ADR 0009: Qt 6 / QML front end on the Rust engine

- **Status:** accepted (supersedes DESIGN.md §6.1's choice of GTK 4 + libadwaita)
- **Date:** 2026-11-15
- **Task:** Phase 1 (decision by the product owner, after a proof)
- **Design reference:** DESIGN.md §1.2 (D1), §6.1, §9.3; ADR 0008

## Context

The product owner wants Cairn to look and move like the FileOS captures in
`fileosmac/`: glass folders, files flying into kind folders, lists that
collapse and reflow, springy controls (ADR 0008). The GTK 4 interface could
match the static look, but motion of that kind would be hand-built, and GTK
cannot blur what is behind a widget.

A proof (`spikes/qt-proof`, commit "P1 spike") showed Qt 6.11 / QML on
Ubuntu 26.10, driven by cairn-core through cxx-qt 0.10:

- cairn-core's GIO listing runs on Qt's event loop (Qt runs GLib's loop on Linux);
- real backdrop blur on the glass folder fronts;
- files flying into kind folders, list collapse and reflow, toast, hover lift;
- the accessibility tree and real key presses work with the Phase 0 harness.

## Decision

- **Front end: Qt 6 with QML**, through **cxx-qt** (Rust ↔ QML). Qt Quick
  Controls (Basic style, fully restyled) for text fields, menus and popups.
- **The engine stays Rust on GIO** (`cairn-core`): listing, Trash, mounts,
  phones, network shares, thumbnails, index. Qt's own file APIs are not used
  for file management.
- **Presenter pattern.** One Rust QObject (`Presenter`) is the only bridge.
  All state and logic stay in plain Rust (`app_state.rs`: navigation,
  selection, grid layout, sidebar, preview), unit-testable without Qt. QML
  renders what the presenter publishes and calls its invokables.
- **Actions stay GIO actions** (`gio::SimpleAction`), exported on D-Bus as
  `org.gtk.Actions` at `/dev/cairn/Files` (app.*) and `/dev/cairn/Files/window/1`
  (win.*). QML shortcuts activate the same actions, so menus, keys, the CLI
  (Phase 7) and tests share one path. The process owns the bus name
  `dev.cairn.Files` (single instance).
- **Window chrome is our own** (frameless window, our header with window
  buttons, system move and resize), because GNOME expects client-side
  decorations and Qt's default ones do not match the look.
- **Icons are our own** simple line icons (SVG, in `crates/app/qml/icons`),
  drawn for this project and recoloured in QML.
- `unsafe_code` is denied (not forbidden) in the workspace: the cxx-qt bridge
  module needs `unsafe extern "C++"` and is the one place allowed to use it.

## Consequences

- The GTK interface code is retired (kept in git history, commit fb3c7b4).
- libadwaita's ready-made pieces (toasts, dialogs, preferences) are rebuilt in
  QML, styled after the captures.
- Ubuntu 24.04 ships Qt 6.4: no `QtQuick.Effects` and no system dark-mode
  reading. Decision deferred to the end of the project (ADR 0007): fall back
  to Qt5Compat effects, or ship 24.04 as a Flatpak with a current Qt.
- The dev image gains Qt (a separate Dockerfile layer). Builds take longer the
  first time (C++ for the bridge).
- End-to-end tests keep their harness. Scenario changes: E0-02 checks the
  dialog kind "about" instead of GTK's type name.

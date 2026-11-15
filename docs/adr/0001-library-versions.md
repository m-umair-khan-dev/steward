# ADR 0001: Library versions and the API baseline

- **Status:** accepted
- **Date:** 2026-11-15
- **Task:** P0-S1
- **Design reference:** DESIGN.md §1.2 (D2), §6.1

## Context

DESIGN.md fixed the API baseline at Ubuntu 24.04 (GTK 4.14, libadwaita 1.5),
marked [verify], and listed the widgets the app relies on. The baseline decides
which gtk4-rs and libadwaita crate features the code may use.

## Findings

Probe: `tests/spikes/probe.py` (S1), run in each dev image.

| Library | Ubuntu 24.04 | Ubuntu 26.10 |
|---|---|---|
| GTK | 4.14.5 | 4.24.1 |
| libadwaita | 1.5.0 | 1.10.0 |
| GtkSourceView | 5.12.0 | 5.22.0 |
| WebKitGTK (6.0 API) | 2.52.6 | 2.52.6 |
| Poppler GLib | 24.02.0 | 26.07.0 |
| GLib | 2.80.0 | 2.90.0 |
| blueprint-compiler | 0.12.0 | 0.20.4 |
| mutter / weston | 46.2 / 13.0.0 | 51.0 / 16.0.0 |
| bubblewrap | 0.9.0 | 0.12.0 |

Widgets DESIGN.md §6.1 relies on, present on **both** releases: AdwApplicationWindow,
AdwOverlaySplitView, AdwNavigationView, AdwNavigationSplitView, AdwToolbarView,
AdwDialog, AdwAlertDialog, AdwAboutDialog, AdwPreferencesDialog, AdwToastOverlay,
AdwStatusPage, AdwBanner; GtkGridView, GtkColumnView, GtkListView,
GtkSectionModel, GtkFileDialog, GtkWidgetPaintable, GtkGraphicsOffload.

**Not** in the 24.04 baseline (present on 26.10): AdwBottomSheet, AdwSpinner,
AdwWrapBox, AdwShortcutsDialog.

Other facts that shape the code:

- GtkGridView has no section headers, even in GTK 4.24. Grouped grids are built
  as a ListView of header rows and card rows (crates/app/src/folder_view.rs).
- The GStreamer development package is not installed; GTK's media support comes
  from the runtime module `libgtk-4-media-gstreamer`, which is what Phase 3
  (Peek) needs.
- Every apt package in `docker/desktop.Dockerfile` exists under the same name on
  24.04, 26.04 and 26.10.

Crates pinned in `Cargo.toml`: gtk4 0.11 with feature `v4_14`, libadwaita 0.9
with feature `v1_5`, Rust 1.99.0.

## Decision

The baseline stands: GTK 4.14 and libadwaita 1.5. The crate features stay at
`v4_14` and `v1_5`, so using a newer API is a compile error. Where a newer widget
would help, use the older equivalent (GtkSpinner instead of AdwSpinner,
GtkShortcutsWindow instead of AdwShortcutsDialog) or gate it at runtime.

## Consequences

- Development and the gate run on 26.10 for now (ADR 0007), but code that
  compiles there also compiles against 24.04's libraries.
- Revisit when 24.04 leaves support or when a needed widget makes it worth it.

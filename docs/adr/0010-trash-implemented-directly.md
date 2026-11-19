# ADR 0010: The Trash is implemented directly, not through GIO

- **Status:** accepted
- **Date:** 2026-11-19
- **Task:** P2-T01, P2-T02
- **Design reference:** DESIGN.md §4 (Trash row: "GIO trash"), §7.1 (undo journal)

## Context

DESIGN.md planned to trash files with GIO (`g_file_trash`) and to undo by
restoring "from Trash by original path". Building the undo journal showed two
problems with that:

- `g_file_trash` does not say where the item went. The Trash may already hold
  an item with the same name, in which case the new one is stored as
  `name.2`, and so on. Finding it again means scanning the Trash and matching
  the original path and a deletion date that has one-second resolution, so
  trashing the same path twice in a second is ambiguous.
- Restoring through `trash:///` needs the GVfs trash daemon. Restoring by
  renaming the item back works without it.

## Decision

`cairn-core::ops::trash` implements the FreeDesktop Trash specification
directly:

- Items on the home filesystem go to `$XDG_DATA_HOME/Trash`; items on other
  filesystems go to `$topdir/.Trash-$uid`.
- The `.trashinfo` file is created exclusively first (it reserves the name),
  then the item is renamed into `files/` with `RENAME_NOREPLACE`.
- The journal records the exact trashed path, so Undo is a rename back that
  never replaces anything and keeps the item's times.

## Consequences

- Items trashed by Cairn appear in Files (Nautilus) and in `trash:///`, and
  items trashed by them appear in Cairn's Trash view: they share the format.
- Cairn does not yet use the `$topdir/.Trash/$uid` variant (an admin-made
  shared `.Trash` with the sticky bit). It uses `.Trash-$uid`, which the
  specification allows as the fallback.
- No `directorysizes` cache is written; the specification makes it optional.

# ADR 0008: Visual direction closer to the reference

- **Status:** accepted
- **Date:** 2026-11-15
- **Task:** Phase 1 (decision by the product owner)
- **Design reference:** DESIGN.md §3.1, §3.2, §3.5; supersedes parts of them

## Context

DESIGN.md §3 chose to look like a stock Ubuntu app: system accent colour,
libadwaita defaults, no "glass" folders. After the first Phase 1 screenshot, the
product owner asked for the interface to look closer to the reference captures
in `fileosmac/` (screenshots and GIFs).

## Decision

Match the reference's visual identity, built from our own drawing and wording:

| Element | Now |
|---|---|
| Accent | Steward purple (`#6B56DC`, `#A897F3` in dark) by default; Preferences › Appearance › "Use system accent colour" switches back |
| Window | Soft lavender background, white rounded content panes |
| Folders | Tinted "glass" folder shapes drawn with widgets and CSS: coloured back with a tab, paper inside, frosted front with item count and a symbol. Colour by kind |
| Files | Cards with a preview area, a coloured type badge, name, "Kind · size" |
| Grid | Always in sections: "Folders N", "Files N". Group by Kind shows "By kind N" as coloured kind folders that open a filtered view |
| Header | Back/forward pill and folder title on the left; segmented grouping (None, Kind, Date) and view (list, grid) controls, preview toggle and menu on the right |
| Breadcrumbs | Their own row under the header, full path, with a "Ctrl+L to type a path" hint |
| Sidebar | App name row; sections Favorites and Locations; purple pill for the selected row; computer row with a usage bar |
| Preview pane | With nothing selected: the folder's size in all, counts, and "What takes the space" by kind |

Still true: libadwaita widgets underneath (dialogs, toasts, preferences,
header bar), system font, symbolic icons from the icon theme, keyboard and
accessibility rules. Nothing is copied from the reference (AGENTS.md rule 12):
no icons, images, logos or text.

## Consequences

- The ungrouped grid is now built like the grouped one (a list of section
  headers and card rows) instead of GtkGridView. Rows are still virtualised.
  GtkGridView's built-in arrow-key navigation and rubber-band selection are
  lost; arrow keys are added to the card rows instead.
- Dark mode needs its own palette (`style-dark.css`), loaded when the style
  manager is dark.
- DESIGN.md §3 is updated to point here.

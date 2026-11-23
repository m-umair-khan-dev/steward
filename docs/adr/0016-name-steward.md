# ADR 0016: The name Steward and the app ID

- **Status:** accepted
- **Date:** 2026-11-23
- **Decision by:** the product owner

## Context

"Cairn" and `dev.cairn.Files` were working names (DESIGN.md, HUMAN list).
The name and the app ID run through the menu entry, the command, the
settings path, the D-Bus names, the package and a future Flathub listing,
so they had to be fixed before the first release.

## Decision

- The app is called **Steward**: it looks after your files the way a
  steward looks after a house (find, keep in order, clear out, keep the
  inventory).
- The app ID is **`io.github.m_umair_khan_dev.steward`** (the product
  owner's GitHub account; Flathub verifies `io.github.` IDs from the
  GitHub login; hyphens become underscores in app IDs). The source is to
  live at `github.com/m-umair-khan-dev/steward`.
- Derived names: D-Bus `io.github.m_umair_khan_dev.steward`, `.Indexer`,
  `.Indexer1`, `.Test1`; object paths under
  `/io/github/m_umair_khan_dev/steward`; commands `steward`,
  `steward-indexer`, `steward-native-host`, `steward-cli`; the package
  `steward` (it conflicts with, replaces and provides `cairn`); data in
  `~/.local/share/steward` and `~/.cache/steward`; gettext domain
  `steward`.
- The icon is a folder holding sorted layers; the default colours match it
  (aubergine on warm cream). Ubuntu's accent and the other colours stay
  available.
- Every internal name follows too: the library crate `steward-core`, the
  `.steward-fixture` safety marker (the guard of AGENTS.md rule 4 works as
  before, only the file name changed), the QML module, the `STEWARD_*`
  development and test variables, the `user.steward.*` extended attributes
  for folder symbols and colours, and the Docker images and volumes. Only
  the package's Conflicts/Replaces/Provides keep `cairn`, so installing
  Steward removes an earlier Cairn test install.

## Consequences

- Settings, the index, bookmarks and folder symbols of a Cairn test
  install are not carried over (nothing was released under the old name).
- Earlier ADRs and documents were updated to the new names.

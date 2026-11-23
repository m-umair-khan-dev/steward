# ADR 0015: The Flatpak build and what works inside it

- **Status:** proposed (the build itself is not yet verified)
- **Date:** 2026-11-22
- **Task:** P9-T01, P9-T02
- **Design reference:** DESIGN.md D6, Phase 9

## Context

DESIGN.md D6 makes the signed .deb the main channel and plans a Flatpak
"later with reduced features". Phase 9 asks for capability detection
(features that cannot work in a sandbox say why) and a Flatpak manifest and
build, marked [verify: needs a privileged builder or a VM in CI].

## Findings

- Out of reach from the sandbox: the host's package tools and databases
  (dpkg, snap, flatpak lists, apt, journalctl). This affects the Installed
  page, Smart Clean's "already installed" installers and leftovers of
  removed apps, and the "Needs administrator" commands.
- Cairn's own `bwrap` sandbox for untrusted file content (thumbnails, text
  for search inside files, ADR 0005) cannot start inside a Flatpak: the
  runtime ships no `bwrap`, and a nested sandbox needs Flatpak's own
  sub-sandbox (`flatpak-spawn --sandbox`). Without it Cairn makes no
  thumbnails and reads no file text there; it already copes with `bwrap`
  missing (both features turn off quietly).
- The manifest needs the KDE 6.10 runtime and SDK with the Rust extension,
  about 4 GB, and network access during `cargo build` (Flathub instead
  needs vendored crates through flatpak-cargo-generator).
- The build was not attempted on the development machine: the disk had
  about 20 GB free and Docker's VM 3.8 GB of memory. Whether
  flatpak-builder runs inside the dev container is still the open
  [verify] question.

## Decision

- Capability detection (P9-T01) is built: `cairn_core::sandbox` detects
  Flatpak (`/.flatpak-info`), Snap (`$SNAP`) or the test switch, and the
  affected features show why. Scenario E9-01.
- The manifest is in `packaging/flatpak/dev.cairn.Files.yml` (P9-T02),
  unbuilt. DESIGN names no fallback for the [verify] item, so the build
  waits for the product owner: run it in CI or a VM, or on a machine with
  room, and fill in the poppler checksum then.
- Thumbnails and search inside files in the Flatpak: follow-up, using
  `flatpak-spawn --sandbox` as the extractor's sandbox when running there.

## Consequences

- E9-02 (build the Flatpak and run the smoke scenarios in it) stays open.
- The HUMAN list gains: the Flatpak build, the Flathub submission and
  review, and the decision on other distributions.

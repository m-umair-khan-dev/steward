# ADR 0015: The Flatpak build and what works inside it

- **Status:** accepted (the bundle builds in Docker; E9-02 still open)
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
- The [verify] item holds: `flatpak-builder` runs in a container
  (2026-11-23, `make flatpak`, Docker Engine). It needs, for that builder
  container only: seccomp and AppArmor unconfined (as the dev container),
  `systempaths=unconfined` (each build step mounts a fresh /proc),
  `SYS_ADMIN` and `NET_ADMIN` (Flatpak checks the exported icon in a
  sandbox with its own network namespace), and `--disable-rofiles-fuse`
  (no /dev/fuse). The runtimes (about 4 GB) stay in a Docker volume.
- The KDE 6.10 SDK's Rust is 1.98; the workspace declares 1.99 (our pinned
  toolchain), so the Flatpak build passes `--ignore-rust-version`. The code
  builds with 1.98.
- Flatpak only accepts whole-segment wildcards for owned D-Bus names, so the
  tray item needs `--own-name=org.kde.*`. Flathub review may refuse that;
  the fix is to register the tray item on the unique bus name.

## Decision

- Capability detection (P9-T01) is built: `cairn_core::sandbox` detects
  Flatpak (`/.flatpak-info`), Snap (`$SNAP`) or the test switch, and the
  affected features show why. Scenario E9-01.
- The manifest is in `packaging/flatpak/dev.cairn.Files.yml` (P9-T02) and
  builds with `make flatpak` into `dist/flatpak/cairn.flatpak`.
- For Flathub (HUMAN: submission): vendor the crates with
  flatpak-cargo-generator (Flathub builds offline), point the source at a
  release tag of the public repository, pass flatpak-builder-lint, and
  move the tray item off a fixed bus name.
- Thumbnails and search inside files in the Flatpak: follow-up, using
  `flatpak-spawn --sandbox` as the extractor's sandbox when running there.

## Consequences

- E9-02 (build the Flatpak and run the smoke scenarios in it) stays open.
- The HUMAN list gains: the Flatpak build, the Flathub submission and
  review, and the decision on other distributions.

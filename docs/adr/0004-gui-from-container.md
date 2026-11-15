# ADR 0004: Showing the app's window from the dev container

- **Status:** accepted
- **Date:** 2026-11-15
- **Task:** P0-S4
- **Design reference:** DESIGN.md §9.3 (`app` service), §9.7 (`make app`)

## Context

`make app` runs the app from the dev container so developers see it while they
work. DESIGN.md planned to bind-mount the host's Wayland socket into the
container, with `GSK_RENDERER=cairo` as the fallback when there is no GPU.

## Findings

On the development machine Docker is **Docker Desktop for Linux**, which runs
containers in a virtual machine. A Unix socket from the host does not cross into
that VM: the mounted path appears as an empty directory, and connecting fails
("Connection refused"). The Wayland approach therefore cannot work on Docker
Desktop, whatever the renderer.

With **Docker Engine** installed natively on Linux, the socket mount is the
normal, documented approach. It was not testable here (no native engine on this
machine).

Fallback found: **GTK's Broadway backend**. `gtk4-broadwayd` (package
`libgtk-4-bin`) serves the app's windows to a web browser over TCP, which Docker
Desktop forwards. Verified: the page at http://127.0.0.1:8085 loads and shows the
running app window (`artifacts/spikes/broadway-24.04.png`, not committed).

Broadway limitations seen: the sidebar of the split view rendered as a thin
strip, while the headless compositor renders it correctly at full width. Treat
Broadway as a convenient viewer for development, not a faithful one.

## Decision

- `make app` detects the Docker engine. On Docker Desktop it runs the
  `app-broadway` service and prints the URL; otherwise the `app` service uses
  the host's Wayland socket.
- Visual checks that matter (and every automated test) use the headless
  compositor, not Broadway.

## Consequences

- README explains both modes.
- HUMAN: a developer with native Docker Engine should run `make app` once and
  confirm the window opens on the desktop.
- Developers on Docker Desktop who want a native window can install Docker
  Engine alongside it and switch context (`docker context use default`).

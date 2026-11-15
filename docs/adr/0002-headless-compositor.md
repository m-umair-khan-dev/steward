# ADR 0002: Headless compositor for tests

- **Status:** accepted
- **Date:** 2026-11-15
- **Task:** P0-S2
- **Design reference:** DESIGN.md §9.3 (`headless-run`), §10.2

## Context

End-to-end tests run the real GTK app in a container with no screen. They need a
Wayland compositor that runs headless. DESIGN.md named mutter first and weston as
the fallback, with the exact flags marked [verify].

## Findings

Probe: `tests/spikes/probe.py` (S2), plus the full end-to-end suite under each.

| Release | mutter | weston |
|---|---|---|
| 24.04 | 46.2: starts in 0.45 s; all Phase 0 scenarios pass | 13.0.0: starts in 0.08 s; all Phase 0 scenarios pass |
| 26.04 | see the 26.04 section below | see below |

Working command lines (`docker/scripts/start-compositor`):

- `mutter --headless --no-x11 --wayland-display=NAME --virtual-monitor 1280x800`
- `weston --backend=headless --socket=NAME --width=1280 --height=800 --idle-time=0`

mutter logs harmless warnings in a container: no system bus, no
`org.gnome.SessionManager`, no locale1 or colord. None affect the app.

mutter renders with a "surfaceless renderer without GPU" (Mesa llvmpipe), so no
GPU is needed in CI.

## Decision

- **mutter is the default** test compositor. It is what Ubuntu users run, and it
  lets tests inject real key presses (ADR 0003).
- **weston stays as the fallback**, selected with `CAIRN_COMPOSITOR=weston`. It
  starts faster but offers no input injection.
- One script, `docker/scripts/start-compositor`, starts either. Both
  `headless-run` and the Python harness use it, so there is one place to change.

## Consequences

- Each end-to-end test starts its own D-Bus session and compositor (DESIGN.md
  §10.2). With mutter, the per-test overhead is under a second.
- `run-e2e` itself does not wrap pytest in `headless-run` (DESIGN.md §9.3 showed
  that); the harness starts a session per test instead, for isolation.

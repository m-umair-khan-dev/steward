# ADR 0003: Accessibility tree and real input in headless tests

- **Status:** accepted
- **Date:** 2026-11-15
- **Task:** P0-S3
- **Design reference:** DESIGN.md §10.2 ("How a test drives and observes the app")

## Context

Tests must read what is on screen and act like a user. DESIGN.md planned the
accessibility tree (AT-SPI) for reading and clicking, and asked whether real key
and pointer input could be injected (Mutter's remote-desktop interface), with
fallbacks for both.

## Findings

Probe: `tests/spikes/probe.py` (S3) and `tests/spikes/followup.py`.

**Accessibility tree: works.** GTK 4's AT-SPI tree is fully visible in the
headless session: window, header bar, menu button, sidebar list, labels, the
About dialog and its buttons. Default actions (`click`) work, so a test can
press a button the way a screen-reader user would (scenario E0-02 does this).

Two practical lessons, now built into the harness:

- AT-SPI role names are not GTK's, and they changed between releases: 24.04
  reports `push button` and `panel`, 26.10 reports `button` and `grouping`. The
  helper treats each pair as the same role; tests use `button` and `panel`.
- Names repeat (the window and a dialog each have a "Close" button), so clicks
  can be limited to a named subtree: `a11y_click("Close", role="push button",
  within="About")`.
- AT-SPI binds to one session bus per process, so each query runs in a short
  helper process (`tests/e2e/harness/atspi_tool.py`) with the test session's
  environment.

**Real key presses: work, with mutter.** `org.gnome.Mutter.RemoteDesktop`:
CreateSession, Start, then NotifyKeyboardKeycode with evdev codes. Finding:
the window is not focused after launch; mutter focuses it on the *first* input
event, and that event is lost. The harness therefore taps Shift first, waits
until the window reports `is_active`, then sends the real keys. Ctrl+Q injected
this way quits the app (harness test `test_harness_presses_real_keys`), on
24.04 and on 26.10.

**Pointer input: not tested.** Absolute pointer motion through RemoteDesktop
needs a ScreenCast stream. Not needed until drag-and-drop tests; revisit then.

## Decision

- Tests read the screen through AT-SPI and click through its actions.
- Tests can press real keys with `app.press("ctrl+q")` (needs mutter).
- Drag and drop and pointer gestures stay on the HUMAN checklists for now, as
  DESIGN.md's fallback says.

## Consequences

- Shortcut scenarios in later phases can press the real keys instead of only
  triggering the action behind them.
- The key table in `tests/e2e/harness/keys.py` assumes a US layout.

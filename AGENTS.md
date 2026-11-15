# Working rules for AI coding agents

These rules come from [DESIGN.md §10.1](DESIGN.md#101-working-rules-for-the-agent).
They apply to every contributor, human or agent.

1. **One phase at a time, tasks in order.** Do not start a phase until the previous phase's gate has passed. One task per commit, with the task ID in the message (`P4-T03: Tidy plan dialog`).
2. **Start and end every session with `docs/PROGRESS.md`.** Read it first, together with the current phase's section in DESIGN.md and the ADRs in `docs/adr/`. Before stopping, record what is done, what is next and what is blocked.
3. **Everything runs through `make` in Docker.** Do not install toolchains on the host. Do not run the app outside the dev container.
4. **Never touch a real home folder.** In development and test builds the app refuses to start unless its home contains the marker file `.cairn-fixture`. Do not remove or bypass that guard.
5. **Write the test with the task.** Each end-to-end scenario is implemented in the same commit series as the feature it covers. A task whose "done when" names a scenario is not done until that scenario passes.
6. **The gate is `make check-all`.** It runs lint, unit and integration tests, and *every* end-to-end scenario written so far, on Ubuntu 24.04 and 26.04. Earlier phases' scenarios are the regression suite; they stay green.
7. **Never weaken a test to make it pass.** If a scenario is wrong, fix the scenario and say why in `docs/PROGRESS.md`. No `sleep` in tests; wait for a condition with a timeout.
8. **When a [verify] item turns out false,** write an ADR in `docs/adr/` with what was found, apply the fallback DESIGN.md names, and continue. If no fallback is named, stop and ask the human.
9. **Stop at steps marked HUMAN.** These need real hardware, a real desktop session, a naming or legal decision, or a secret. List them in `docs/PROGRESS.md` and hand over.
10. **All file changes go through `cairn-core::ops`.** No other module may move, copy, rename, trash or delete. A static check enforces this (scenario E2-13).
11. **Keep the test hooks stable.** Every user command is a named `GAction`. Every interactive widget has an accessible name. All user-visible strings go through gettext. Tests run with `LANG=C.UTF-8`.
12. **Do not copy from the reference.** No text, icons, images or demo data from `fileosmac/`. It is there to look at.

## Quick reference

| Command | What it does |
|---|---|
| `make init` | First-time setup |
| `make app` | Run the app against the fake home |
| `make test` | Unit and integration tests |
| `make e2e` | End-to-end tests (`make e2e k=E0_03` for one) |
| `make lint` | Formatting, clippy, Dockerfile and Compose checks |
| `make check` | The gate on the default Ubuntu release |
| `make check-all` | The gate on 24.04 and 26.04 |
| `make shell` | A shell with the full toolchain |

When an end-to-end test fails, read `artifacts/e2e/<test-id>/` (screenshot,
accessibility tree, app state, app log) before changing code.

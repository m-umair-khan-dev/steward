# Contributing

Thanks for helping. This project is built in the open, by people and by AI
coding agents working under the same rules.

## Before you start

1. Read [DESIGN.md](DESIGN.md), at least §1 (decisions) and §10 (the plan).
2. Read [AGENTS.md](AGENTS.md). Its rules apply to everyone, not only agents.
3. Check [docs/PROGRESS.md](docs/PROGRESS.md) for what is in progress.

## Setting up

All development happens in Docker. You need Docker with Compose and `make`.

```sh
make init
make app
```

## Workflow

- Work on one task from the current phase in [DESIGN.md §10](DESIGN.md#10-implementation-plan-phases-and-end-to-end-tests).
- One task per commit; put the task ID first in the message, e.g. `P1-T03: grid and list views`.
- Write or update the end-to-end scenario for the task in the same change.
- Run `make check` before opening a pull request. CI runs `make check-all`
  (both supported Ubuntu releases).

## Decisions

Anything that changes the architecture, or a finding that contradicts DESIGN.md,
gets an Architecture Decision Record in [docs/adr/](docs/adr/). Copy the template.

## Code of conduct

Be kind, be specific, assume good intent.

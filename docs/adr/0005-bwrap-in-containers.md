# ADR 0005: The bubblewrap sandbox inside containers

- **Status:** accepted
- **Date:** 2026-11-15
- **Task:** P0-S5
- **Design reference:** DESIGN.md §7.1 (`extract`), §8.5

## Context

Thumbnailers and text extractors run untrusted file content, so DESIGN.md puts
them in a `bwrap` sandbox with no network. The question was whether bwrap works
inside the dev and CI containers. The named fallback was to run extractors
unsandboxed in containers only, behind a test-only switch.

## Findings

Probe: `tests/spikes/probe.py` (S5) and `tests/spikes/followup.py`. Containers
run with `seccomp=unconfined` (compose.desktop.yaml), which permits user
namespaces.

| bwrap options | Result in the container (24.04) |
|---|---|
| `--unshare-all --proc /proc` | **Fails**: "Can't mount proc on /newroot/proc: Operation not permitted". Docker masks parts of the container's /proc, so a fresh proc mount is refused |
| `--unshare-all`, no /proc | Works; network blocked |
| `--unshare-all --ro-bind /proc /proc` | Works; network blocked |
| user, net, ipc, uts namespaces, no pid namespace | Works; network blocked |

## Decision

- The sandbox profile **does not mount /proc**. Extractors and thumbnailers do
  not need it, and leaving it out is also stricter. With this profile bwrap
  works the same in containers and on real systems.
- The unsandboxed fallback is **not needed** and will not be built.

## Consequences

- Phase 1 (thumbnails) and Phase 7 (extraction) use one profile everywhere:
  `--unshare-all --die-with-parent`, read-only `/usr`, `--dev /dev`, the one
  input file bound read-only, no `/proc`.
- `seccomp=unconfined` remains a dev- and test-only setting.

## Addendum (2026-11-23): Docker Engine

On Docker Engine (Ubuntu's docker.io) the container also runs under Docker's
default AppArmor profile, which refuses bwrap's mounts ("Failed to make /
slave: Permission denied"); Docker Desktop's VM has no AppArmor, so this did
not show before. The dev and test services now add `apparmor=unconfined`
next to `seccomp=unconfined`. Both stay dev- and test-only (compose.desktop.yaml).

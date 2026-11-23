# Security

Steward works with your files, so a bug that lets something read, change or
delete files it should not is a security problem.

## Reporting

Please report security problems **privately**, not in a public issue:
write to the maintainers at `security@example.invalid` (HUMAN: the real
address once the project has a host). Say what you found, how to reproduce
it, and which version and Ubuntu release you used.

You will get an answer within a week. Once a fix is released, the report is
credited in the changelog unless you prefer not.

## Supported versions

The latest release on each supported Ubuntu release gets security fixes.

## What Steward does to stay safe

- Every change to your files goes through one module (`steward-core::ops`) and
  can be undone; a static check enforces this (scenario E2-13).
- Nothing runs as administrator. Commands that need it are shown to copy.
- No network access except opening network locations you choose; no
  accounts, no telemetry.
- Packages are signed; the repository's key fingerprint is on the download
  page.

# ADR 0007: Release targets during development

- **Status:** accepted
- **Date:** 2026-11-15
- **Task:** Phase 0 (decision by the product owner)
- **Design reference:** DESIGN.md §1.2 (D2), §10.1 rule 6, §8.7

## Context

DESIGN.md set the gate to run on Ubuntu 24.04 and 26.04 on every task. Phase 0
built images for both and passed lint, tests and end-to-end tests on 24.04.

The product owner decided to develop and validate the whole project on
**Ubuntu 26.10** first (the release on the team's desktops), and to test on the
earlier releases only once the project is complete.

## Decision

- The gate (`make check-all`, CI) runs on **26.10 only** until the project is
  complete. `make` defaults to 26.10.
- 24.04 and 26.04 stay buildable (`make check UBUNTU_VERSION=24.04`), with
  pinned images, but are not part of the gate.
- **The code stays within the 24.04 baseline APIs**: the gtk4-rs feature
  `v4_14` and the libadwaita feature `v1_5` stay as they are, so code that uses a
  newer widget fails to compile. This keeps the final step (adding the older
  releases back) small.
- A final task, after Phase 9, adds 24.04 and 26.04 back to the gate and fixes
  whatever differs.

## Consequences

- Faster feedback: one build cache and one test run per change instead of two.
- Risk: behaviour that differs between releases (for example mutter and weston
  versions, or package names) is found late. The pinned-API rule limits it to
  runtime behaviour, not compile errors.
- 26.10 is an interim release with nine months of support. Release builds still
  target the LTS releases as DESIGN.md says; this ADR only changes the order of
  testing.
- Phase 0 evidence for 24.04 (ADRs 0001–0006) was gathered before this decision
  and is kept.

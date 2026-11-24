# Releasing

Only the owner merges into `main`. A ruleset on `main` requires a pull
request and a green `gate` check (`.github/workflows/ci.yml`), with the
branch up to date before merging. The owner can bypass it.

## What runs where

| When | Workflow | What it does |
|---|---|---|
| Every pull request, every push to `main` | CI | Lint, unit and integration tests, every end-to-end scenario (26.10); the .deb for 24.04, 26.04 and 26.10, installed in a clean Ubuntu. `gate` is green only if all of it passed |
| Every push to `main` | Release | Nothing, unless `Cargo.toml` holds a version that has no tag yet |

## Making a release

1. In a pull request, raise `version` in `Cargo.toml` (for example
   1.0.0 → 1.0.1), refresh `Cargo.lock` (`make lockfile`), and add a
   `<release version="1.0.1" date="…">` entry to
   `data/io.github.m_umair_khan_dev.steward.metainfo.xml`.
2. Merge it once `gate` is green.
3. The Release workflow then:
   - builds and install-tests the .deb for each Ubuntu release,
   - tags `v1.0.1` and publishes the GitHub Release with the .debs,
     `SHA256SUMS` and notes (install steps, then the merged pull requests),
   - builds the snap and uploads it to the edge channel (see
     [snap.md](snap.md#from-ci)).
4. By hand: move the snap to stable (`snapcraft release steward <rev> stable`).

If the release workflow stops because the metainfo has no entry for the
new version, add it in a new pull request; the release runs on that merge.

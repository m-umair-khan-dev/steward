# Cairn

A file manager for Ubuntu that keeps your folders tidy: Downloads sorted in one
click, disk space recovered with a preview first, a project's folders on one
page, two folders side by side. Everything it changes can be undone.

> **Status: early development (Phase 0).** Nothing here is usable yet.
> "Cairn" is a working name.

- Design and implementation plan: [DESIGN.md](DESIGN.md)
- Progress: [docs/PROGRESS.md](docs/PROGRESS.md)
- Contributing: [CONTRIBUTING.md](CONTRIBUTING.md)

## Quick start (developers)

You need Docker (with Compose) and `make`. Nothing else is installed on your machine.

```sh
make init     # build the images, create the fake home folder the app runs against
make app      # run the app (see "Seeing the window" below)
make check    # lint, tests and end-to-end tests
```

### Seeing the window

- **Docker Engine on Linux:** the window opens on your desktop.
- **Docker Desktop:** it cannot reach your desktop's display, so the window is
  shown in your browser instead at <http://127.0.0.1:8085>.

The app always runs against a generated fake home folder, never your real files.

## Licence

GPL-3.0-or-later. See [LICENSE](LICENSE).

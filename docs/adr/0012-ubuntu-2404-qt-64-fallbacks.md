# ADR 0012: Ubuntu 24.04 (Qt 6.4) through run-time feature checks

- **Status:** accepted
- **Date:** 2026-11-20
- **Task:** decision by the product owner (a 24.04 build to share for testing);
  settles the question ADR 0009 deferred
- **Design reference:** ADR 0007, ADR 0009, DESIGN.md §8.8

## Context

Ubuntu 24.04 LTS (GNOME 46) ships Qt 6.4.2. Steward was built on Qt 6.11.
ADR 0009 noted two gaps and deferred the choice between Qt5Compat effects and
a Flatpak.

- **QtQuick.Effects (Qt 6.5+).** It provides the glass blur, the soft
  shadows and icon tinting, and was imported by 11 QML files. In QML a
  missing import fails the whole file, so on 24.04 those screens would not
  load at all.
- **`Qt.styleHints.colorScheme` (Qt 6.5+).** Qt 6.4 cannot report the
  system's dark mode.

The product owner chose to make the app work on Qt 6.4 with fallbacks, and
asked that newer releases keep the full look: check at run time, and fall
back only where a feature is missing.

## Findings

- Steward and its C++ bridge (cxx-qt 0.10) build against Qt 6.4.2 without
  changes.
- **Theme was not a singleton on Qt 6.4.** The QML files live in a `qml/`
  folder inside the module. On Qt 6.4 a file's implicit import of its own
  folder takes precedence over the module, and that folder had no `qmldir`,
  so `Theme` was a plain type and every token was undefined. Qt 6.5+
  resolves it through the module.
- **Switches and accessibility.** On Qt 6.4 the accessibility actions are
  named "Toggle" and "Press", and a Toggle does not emit `toggled()`.
- **Earlier shader cache.** Qt 6.4 writes its shader cache into `~/.cache`
  at start-up.
- **New files arrive later.** Older GLib reports new files to a folder
  through its file monitor a moment later.

## Decision

- **Feature check.** At start-up Main.qml tries to create a `MultiEffect`.
  - If that works (Qt 6.5+), `Theme.effects` is true and everything is
    drawn exactly as before.
  - If it fails (Qt 6.4), the fallbacks below are used.
  - `STEWARD_LEGACY_FX=1` forces the fallbacks on any Qt, for testing and
    screenshots.
- **Effects behind wrappers.** The three effect files (`qml/fx/Shadow.qml`,
  `Tint.qml`, `Blur.qml`) are outside the QML module, in `qml/fx.qrc`, so Qt
  6.4 never compiles or loads them. Screens use `FxShadow`, `FxBlur` and
  `Icon`, which load them only when `Theme.effects` is true.
- **The fallbacks:**
  - shadow layers are off;
  - the glass front is translucent without blur;
  - icons are recoloured in the SVG itself (`p.svgicon`, from icons compiled
    into the binary).
- **Dark mode.** Qt's own value where it exists; otherwise GNOME's
  `org.gnome.desktop.interface color-scheme` (`prefs.systemDark`), followed
  live.
- **Theme on Qt 6.4.** A `qmldir` in the `qml/` folder declares Theme a
  singleton (`qml/local.qrc`); it changes nothing on Qt 6.5+.
- **Preferences switches** act on any change of `checked` and write only
  when it differs from the setting, so the accessibility Toggle works on
  Qt 6.4 too.
- **Packaging.** The image installs Qt 6.5-only packages where they exist.
  The `.deb` has a cargo-deb variant `ubuntu2404` whose dependencies leave
  out `qml6-module-qtquick-effects` and use `libqt6svg6` for the SVG plugin.
  `packaging/build-deb` picks it on 24.04.
- **Harness changes:**
  - Qt's shader disk cache is off in test sessions, like Mesa's: it belongs
    to the machine, not the test home.
  - AT-SPI action names are matched without regard to case.
  - E2-03 waits for the pasted copies to be selected instead of reading the
    selection once.

## Consequences

- One code base and one `.deb` recipe serve 24.04, 26.04 and 26.10.
- On 24.04 the look is simpler: no blur and no soft shadows. Everything else
  is the same.
- The gate still runs on 26.10 only (ADR 0007). 24.04 was checked by hand
  with the full end-to-end suite and `make deb-test UBUNTU_VERSION=24.04`.
  Adding it to `make check-all` doubles the gate's time; that is the product
  owner's choice.

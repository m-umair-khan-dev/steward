# Recorded tool output

Output of the package tools as Cairn runs them (P5-T08), for the parser tests
in `crates/core/tests/packages.rs` and the stand-in tools of E5-08 and E5-09.
Written for this project in the format each tool prints; the packages are
ordinary names and the versions are made up.

| File | Command |
|---|---|
| `dpkg-query-list.txt` | `dpkg-query -W -f='${Package}\t${Version}\t${Installed-Size}\t${db:Status-Status}\t${binary:Summary}\n'` |
| `dpkg-query-owners.txt` | `dpkg-query -S /usr/share/applications` |
| `apt-mark-showmanual.txt` | `apt-mark showmanual` |
| `snap-list.txt` | `snap list` |
| `flatpak-apps.txt`, `flatpak-runtimes.txt` | `flatpak list --app/--runtime --columns=application,version,size,installation` |
| `npm-ls.json` | `npm ls -g --depth=0 --json` |
| `pipx-list.json` | `pipx list --json`; Cairn reads the same metadata from each venv's `pipx_metadata.json` instead of running pipx, which would log the run |
| `pip-list.json`, `pip-list-not-required.json` | `pip list --user [--not-required] --format=json` |
| `cargo-install-list.txt` | `cargo install --list` |
| `gem-list.txt` | `gem list --local` |
| `brew-list-formula.txt`, `brew-leaves.txt` | `brew list --formula --versions`, `brew leaves` |
| `go-version-m.txt` | `go version -m <binary>` |
| `expected.json` | What E5-08 expects per tool |
| `installed-list.golden.txt` | What Save List writes for E5-08 |

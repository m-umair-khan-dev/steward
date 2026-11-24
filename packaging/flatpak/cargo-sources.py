#!/usr/bin/env python3
"""Writes cargo-sources.json for the Flathub build from Cargo.lock.

Flathub builds without network, so every crate is a flatpak-builder source:
the .crate archive from static.crates.io (checked against the checksum in
Cargo.lock), unpacked into cargo/vendor/<name>-<version>, with the
.cargo-checksum.json cargo wants beside it, and a cargo config that points
crates.io at that folder. Same output shape as flatpak-builder-tools'
flatpak-cargo-generator, for crates.io-only lock files.

Usage: packaging/flatpak/cargo-sources.py Cargo.lock > cargo-sources.json
"""

import json
import sys
import tomllib

VENDOR = "cargo/vendor"
CRATES_IO = "registry+https://github.com/rust-lang/crates.io-index"


def main(lock_path: str) -> None:
    with open(lock_path, "rb") as f:
        lock = tomllib.load(f)
    sources = []
    for pkg in lock["package"]:
        source = pkg.get("source")
        if source is None:
            continue  # a crate of this workspace
        if source != CRATES_IO:
            sys.exit(f"{pkg['name']}: only crates.io sources are handled, got {source}")
        name, version, checksum = pkg["name"], pkg["version"], pkg["checksum"]
        dest = f"{VENDOR}/{name}-{version}"
        sources.append({
            "type": "archive",
            "archive-type": "tar-gzip",
            "url": f"https://static.crates.io/crates/{name}/{name}-{version}.crate",
            "sha256": checksum,
            "dest": dest,
        })
        sources.append({
            "type": "inline",
            "contents": json.dumps({"package": checksum, "files": {}}),
            "dest": dest,
            "dest-filename": ".cargo-checksum.json",
        })
    sources.append({
        "type": "inline",
        "contents": (
            "[source.vendored-sources]\n"
            f'directory = "{VENDOR}"\n\n'
            "[source.crates-io]\n"
            'replace-with = "vendored-sources"\n'
        ),
        "dest": "cargo",
        "dest-filename": "config.toml",
    })
    json.dump(sources, sys.stdout, indent=4)
    sys.stdout.write("\n")


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else "Cargo.lock")

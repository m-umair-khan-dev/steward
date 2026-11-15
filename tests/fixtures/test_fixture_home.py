"""Tests for the fixture-home generator (P0-T05)."""

import json
import os
import subprocess
import sys
from pathlib import Path

import pytest

SCRIPT = Path(__file__).resolve().parents[2] / "docker" / "scripts" / "make-fixture-home"


def run(*args: str, check: bool = True) -> subprocess.CompletedProcess:
    return subprocess.run([sys.executable, "-I", str(SCRIPT), *args], capture_output=True,
                          text=True, check=check)


@pytest.fixture(scope="module")
def built(tmp_path_factory) -> Path:
    home = tmp_path_factory.mktemp("home-a")
    run(str(home))
    return home


def manifest_of(home: Path) -> dict:
    return json.loads((home / ".cairn-fixture").read_text())


def test_same_recipe_gives_identical_trees(built, tmp_path):
    second = tmp_path / "home-b"
    run(str(second))
    assert run("--digest", str(built)).stdout == run("--digest", str(second)).stdout


def test_marker_is_a_file_holding_the_manifest(built):
    marker = built / ".cairn-fixture"
    assert marker.is_file()
    assert manifest_of(built)["recipe_version"] >= 1


def test_manifest_file_count_matches_the_tree(built):
    files = [p for p in built.rglob("*") if p.is_file()]
    assert manifest_of(built)["totals"]["files"] == len(files)


def test_manifest_folder_counts_match_the_tree(built):
    for name, facts in manifest_of(built)["folders"].items():
        children = list((built / name).iterdir())
        assert facts["items"] == len(children), name
        assert facts["files"] == sum(1 for c in children if c.is_file()), name


def test_tidy_plan_skips_partial_downloads_and_folders(built):
    manifest = manifest_of(built)
    sources = {move["from"] for move in manifest["tidy"]["moves"]}
    assert "Downloads/Unconfirmed 418235.crdownload" not in sources
    assert "Downloads/trip-edit.mp4.part" not in sources
    assert all((built / source).is_file() for source in sources)
    assert set(manifest["tidy"]["left"]) == {"Unconfirmed 418235.crdownload", "trip-edit.mp4.part"}


def test_planted_duplicates_are_reported(built):
    groups = [set(g) for g in manifest_of(built)["duplicates"]]
    assert {"Downloads/Invoice-0923.pdf", "Downloads/Invoice-0923 (1).pdf",
            "Documents/Invoices/Invoice-0923.pdf"} in groups
    assert {"Downloads/logo-final.png", "Downloads/logo-final (1).png"} in groups


def test_big_files_are_sparse(built):
    iso = built / "Downloads" / "distro-26.04-desktop-amd64.iso"
    assert iso.stat().st_size == 6 * 1024**3
    assert iso.stat().st_blocks * 512 < 1024**2  # occupies well under a megabyte


def test_files_have_fixed_times(built):
    sunset = (built / "Downloads" / "beach-sunset.jpg").stat()
    now = 1793534400  # 2026-11-01T12:00:00Z, the recipe's "now"
    assert sunset.st_mtime_ns == (now - 12 * 86400) * 10**9


def test_refuses_to_clear_a_folder_that_is_not_a_fixture(tmp_path):
    real = tmp_path / "real-home"
    real.mkdir()
    (real / "thesis.odt").write_text("precious")
    result = run(str(real), "--force", check=False)
    assert result.returncode != 0
    assert "refusing" in result.stderr
    assert (real / "thesis.odt").read_text() == "precious"


def test_refuses_a_non_empty_folder_without_force(built):
    result = run(str(built), check=False)
    assert result.returncode != 0


def test_force_rebuilds_an_existing_fixture(built, tmp_path):
    home = tmp_path / "rebuild"
    run(str(home))
    (home / "Downloads" / "stray.txt").write_text("left over")
    run(str(home), "--force")
    assert not (home / "Downloads" / "stray.txt").exists()
    assert run("--digest", str(home)).stdout == run("--digest", str(built)).stdout


def test_accepts_a_fresh_user_skeleton(tmp_path):
    home = tmp_path / "skeleton"
    home.mkdir()
    for name in (".bashrc", ".profile"):
        (home / name).write_text("# skeleton")
    run(str(home), "--force")
    assert (home / ".cairn-fixture").is_file()
    assert not (home / ".bashrc").exists()


@pytest.mark.skipif(not os.path.exists("/usr/bin/dpkg-deb"), reason="dpkg-deb not installed")
def test_planted_deb_is_readable_by_dpkg(built):
    deb = built / "Downloads" / "photo-tool_3.2.0_amd64.deb"
    out = subprocess.run(["dpkg-deb", "-f", str(deb), "Package", "Version"], capture_output=True,
                         text=True, check=True).stdout
    assert "photo-tool" in out and "3.2.0" in out

"""Phase 7 end-to-end scenarios: developer tools (E7-01 to E7-03)."""

import pytest

pytestmark = pytest.mark.p7

WEB_APP = "Documents/Projects/web-app"


def git(app):
    return app.state()["path_bar"]["git"]


def test_E7_01_the_git_chip(app, home):
    facts = home.manifest["git"][WEB_APP]
    project = home / WEB_APP
    before = home.snapshot(WEB_APP)
    app.open_folder(project)
    chip = app.wait_for(lambda: git(app), "the Git chip")
    assert chip["branch"] == facts["branch"] and chip["changes"] == facts["changes"]
    assert f"Git branch {facts['branch']}, {facts['changes']} uncommitted changes" in app.a11y_names()
    app.open_folder(project / "src")
    app.wait_for(lambda: (g := git(app)) and g["root"] == str(project), "the chip in a subfolder")
    app.wait_idle()
    assert home.snapshot(WEB_APP) == before, "the repository is byte-for-byte unchanged"


def test_E7_02_no_chip_and_no_crash(app, home):
    app.open_folder(home / "Documents" / "Invoices")
    app.wait_idle()
    assert git(app) is None, "not a repository"
    school = home / "Documents" / "School"
    (school / ".git").mkdir()
    (school / ".git" / "HEAD").write_text("not a ref at all\n")
    app.open_folder(school)
    app.wait_idle()
    assert git(app) is None, "a corrupt .git gives no chip"
    assert app.running()

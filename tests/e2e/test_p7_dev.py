"""Phase 7 end-to-end scenarios: developer and content tools (E7-01 onwards)."""

import subprocess
from pathlib import Path

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


def install_fake_app(home, fakes, desktop_id: str, program: str, name: str) -> None:
    """A stand-in app: a desktop file in the test home whose Exec is a fake
    program that logs how it was started."""
    fakes.tool(program)
    apps = home / ".local" / "share" / "applications"
    apps.mkdir(parents=True, exist_ok=True)
    (apps / desktop_id).write_text(f"[Desktop Entry]\nType=Application\nName={name}\n"
                                   f"Exec={fakes.folder / program} %F\n")


def test_E7_03_open_in_terminal_and_editor(session, home, app_factory, tmp_path):
    from harness.fakes import FakeTools

    fakes = FakeTools(tmp_path / "fake-bin")
    install_fake_app(home, fakes, "org.gnome.Terminal.desktop", "gnome-terminal", "Terminal")
    install_fake_app(home, fakes, "code.desktop", "code", "Visual Studio Code")
    app = app_factory()
    info = app.state()["openin"]
    assert {t["id"] for t in info["terminals"]} == {"gnome-terminal"}
    assert {e["id"] for e in info["editors"]} == {"vscode"}
    project = home / WEB_APP
    app.open_folder(project)

    app.press("f4")
    app.wait_for(lambda: fakes.calls("gnome-terminal"), "the terminal to start")
    assert fakes.calls("gnome-terminal")[-1] == [f"--working-directory={project}"]

    app.press("ctrl+l")
    app.wait_for(lambda: app.state()["path_bar"]["editing"], "the path field")
    app.type_text("code .")
    app.press("return")
    app.wait_for(lambda: fakes.calls("code"), "the editor to start")
    assert fakes.calls("code")[-1] == [str(project)]
    assert app.folder()["path"] == str(project), "the command did not navigate"


def test_the_item_menu_opens_from_the_keyboard(app, home):
    """Shift+F10 (and the Menu key) open the selection's menu, with Open In."""
    app.open_folder(home / "Documents" / "Projects")
    app.select("web-app")
    app.press("shift+f10")
    app.wait_for(lambda: {"Open in Terminal", "Open in Editor"} <= app.a11y_names(), "the item menu")


# --- AI Agents (E7-04, E7-05) ----------------------------------------------------------

from gi.repository import GLib  # noqa: E402


def s(text: str) -> GLib.Variant:
    return GLib.Variant("s", text)


def agents(app) -> dict:
    return app.state()["agents"]


def open_agents(app) -> dict:
    app.action("win.tool-open", s("agents"))
    return app.wait_for(lambda: (a := agents(app)) and a["loaded"] and a, "AI Agents", timeout=20)


def test_E7_04_ai_agents_match_the_manifest(session, home, app_factory):
    app = app_factory(extra_env={"CAIRN_FAKE_NOW": home.manifest["now"]})
    expected = home.manifest["agents"]
    state = open_agents(app)
    projects = {str(__import__("pathlib").Path(p["path"]).relative_to(home.path)): p for p in state["projects"]}
    assert set(projects) == set(expected["projects"])
    names = {"claude": "Claude Code", "codex": "Codex", "cursor": "Cursor"}
    for rel, facts in expected["projects"].items():
        got = projects[rel]
        assert got["tools"] == [names[t] for t in facts["tools"]], rel
        assert got["files"] == facts["files"], rel
        assert sorted(s["name"] for s in got["servers"]) == sorted(facts["servers"]), rel
        assert got["tokens"] == facts["tokens"], rel
    assert state["user_servers"] == len(expected["user_servers"])
    assert state["usage_total"] == expected["tokens_total"], "this week's tokens"
    assert "web-app" in app.a11y_names(showing_only=False)


def free_port() -> int:
    import socket
    with socket.socket() as probe:
        probe.bind(("127.0.0.1", 0))
        return probe.getsockname()[1]


def test_E7_05_stop_a_local_server(session, home, app_factory):
    import subprocess
    import sys
    port = free_port()
    server = subprocess.Popen([sys.executable, "-m", "http.server", str(port), "--bind", "127.0.0.1"],
                              cwd=home / WEB_APP, env=session.base_env(),
                              stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    try:
        app = app_factory()
        open_agents(app)
        listed = app.wait_for(lambda: next((x for x in agents(app)["servers"] if x["port"] == port), None),
                              "the server to be listed", timeout=15)
        assert listed["project"] == "web-app" and listed["pid"] == server.pid

        app.a11y_click(f"Stop the server on port {port}")
        app.wait_for(lambda: (d := app.state()["active_window"]["dialog"]) and d["type"] == "stop-server", "the question")
        app.a11y_click("Cancel", within="Stop the Server?")
        app.wait_for(lambda: app.state()["active_window"]["dialog"] is None, "the question to close")
        assert server.poll() is None, "cancel leaves it running"

        app.a11y_click(f"Stop the server on port {port}")
        app.wait_for(lambda: (d := app.state()["active_window"]["dialog"]) and d["type"] == "stop-server", "the question")
        app.a11y_click("Stop", role="button", within="Stop the Server?")
        app.wait_for(lambda: server.poll() is not None, "the server to end", timeout=10)
        app.wait_for(lambda: all(x["port"] != port for x in agents(app)["servers"]), "the list to update", timeout=10)
    finally:
        if server.poll() is None:
            server.kill()


# --- Markdown (E7-06) -------------------------------------------------------------------

TINY_PNG = ("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNk+M9QDwADhgGAWjR9awAAAABJRU5ErkJggg==")


class Watcher:
    """A local web server that only counts the requests it gets."""

    def __init__(self) -> None:
        import http.server
        import threading
        self.requests: list[str] = []
        watcher = self

        class Handler(http.server.BaseHTTPRequestHandler):
            def do_GET(self):  # noqa: N802
                watcher.requests.append(self.path)
                self.send_response(404)
                self.end_headers()

            def log_message(self, *args):
                pass

        self.server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.port = self.server.server_address[1]
        threading.Thread(target=self.server.serve_forever, daemon=True).start()

    def close(self) -> None:
        self.server.shutdown()
        self.server.server_close()


def md(app) -> dict:
    return app.state()["markdown"]


def test_E7_06_markdown_view(app, home, tmp_path):
    import base64
    watcher = Watcher()
    try:
        notes = home / "Documents" / "Notes"
        notes.mkdir(exist_ok=True)
        (notes / "pic.png").write_bytes(base64.b64decode(TINY_PNG))
        url = f"http://127.0.0.1:{watcher.port}"
        doc = notes / "guide.md"
        original = (f"# Guide\n\nRead this first.\n\n![tracker]({url}/pixel.png)\n\n"
                    f"<img src=\"{url}/tag.png\">\n\n![ref][r]\n\n[r]: {url}/ref.png\n\n"
                    "## Install\n\n```\n# not a heading\n```\n\n![local](pic.png)\n\n## Use\n\nDone.\n")
        doc.write_text(original)

        app.open_folder(notes)
        app.select("guide.md")
        app.press("return")
        app.wait_for(lambda: app.state()["tools"]["page"] == "markdown", "the Markdown page")
        state = md(app)
        assert state["path"] == str(doc) and state["mode"] == "preview"
        assert state["outline"] == ["Guide", "Install", "Use"]
        import re
        images = re.findall(r"!\[[^\]]*\]\([^)]*\)|<img[^>]*>", state["preview"], re.IGNORECASE)
        assert images and not any(str(watcher.port) in i for i in images), \
            f"remote images are removed before rendering: {images}"
        assert f"file://{notes / 'pic.png'}" in state["preview"], "a local image beside the file stays"
        names = app.a11y_names()
        assert {"Outline", "Guide", "Install", "Use", "Preview"} <= names

        # Edit: type a new section, save.
        app.a11y_click("Edit", within="View")
        app.wait_for(lambda: md(app)["mode"] == "edit", "Edit")
        app.a11y_focus("Markdown text")
        app.press("ctrl+end")
        app.type_text("\n## Added\n")
        app.wait_for(lambda: md(app)["modified"] and "Added" in md(app)["outline"], "the edit and the outline")
        app.press("ctrl+s")
        app.wait_for(lambda: not md(app)["modified"], "Save")
        saved = doc.read_text()
        assert saved == original + "\n## Added\n", "Save writes the text as typed"

        # Revert: type again, then go back to the saved file.
        app.type_text("more")
        app.wait_for(lambda: md(app)["modified"], "the second edit")
        app.a11y_click("Revert", role="button")
        app.wait_for(lambda: not md(app)["modified"], "Revert")
        assert md(app)["buffer"] == saved and doc.read_text() == saved

        # Split shows both.
        app.a11y_click("Split", within="View")
        app.wait_for(lambda: md(app)["mode"] == "split", "Split")
        assert {"Markdown text", "Preview"} <= app.a11y_names()

        # Everything has been rendered: no request ever reached the server.
        app.screenshot(tmp_path / "split.png")
        app.wait_idle()
        assert watcher.requests == [], f"the preview loaded {watcher.requests}"
    finally:
        watcher.close()


# --- Bookmarks (E7-07) ------------------------------------------------------------------

class Site(Watcher):
    """A local web site: a page with a title and a picture, and a second page."""

    def __init__(self) -> None:
        import base64
        import http.server
        import threading
        self.requests: list[str] = []
        site = self
        png = base64.b64decode(TINY_PNG)
        pages = {
            "/calm.html": ("text/html; charset=utf-8",
                           b"<html><head><title>Fallback</title>"
                           b"<meta property='og:title' content='Designing calm interfaces'>"
                           b"<meta property='og:image' content='/card.png'></head><body>Hi</body></html>"),
            "/card.png": ("image/png", png),
            "/second.html": ("text/html", b"<title>Second page</title>"),
        }

        class Handler(http.server.BaseHTTPRequestHandler):
            def do_GET(self):  # noqa: N802
                site.requests.append(self.path)
                kind, body = pages.get(self.path, ("text/plain", b""))
                self.send_response(200 if self.path in pages else 404)
                self.send_header("Content-Type", kind)
                self.send_header("Content-Length", str(len(body)))
                self.end_headers()
                self.wfile.write(body)

            def log_message(self, *args):
                pass

        self.server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), Handler)
        self.port = self.server.server_address[1]
        threading.Thread(target=self.server.serve_forever, daemon=True).start()


def links(app) -> list[dict]:
    return app.state()["bookmarks"]["all"]


def test_E7_07_bookmarks(app, app_factory, home):
    site = Site()
    try:
        first = f"http://127.0.0.1:{site.port}/calm.html"
        second = f"http://127.0.0.1:{site.port}/second.html"
        app.action("win.bookmarks-show", s("all"))
        app.wait_for(lambda: app.state()["tools"]["page"] == "bookmarks", "Bookmarks")

        # With fetching on: the card gets the page's title and picture.
        app.action("win.bookmark-add", s(first))
        got = app.wait_for(lambda: (b := links(app)) and b[0]["fetched"] and b[0], "the fetch", timeout=15)
        assert got["url"] == first and got["title"] == "Designing calm interfaces"
        assert got["image_exists"], "the picture was saved"
        assert sorted(site.requests) == ["/calm.html", "/card.png"]
        app.wait_for(lambda: "Designing calm interfaces" in app.a11y_names(), "the card")

        # Fetching off (Preferences): no request at all; the card shows the address.
        app.action("app.preferences")
        app.wait_for(lambda: app.state()["active_window"]["dialog"], "Preferences")
        app.a11y_click("Fetch link titles and pictures", within="Preferences")
        app.wait_for(lambda: not app.state()["bookmarks"]["fetch"], "fetching off")
        app.press("escape")
        app.wait_for(lambda: not app.state()["active_window"]["dialog"], "Preferences to close")
        app.action("win.bookmark-add", s(second))
        app.wait_for(lambda: len(links(app)) == 2, "the second link")
        app.wait_idle()
        plain = next(b for b in links(app) if b["url"] == second)
        assert plain["title"] is None and not plain["fetched"] and plain["image"] is None
        assert app.state()["bookmarks"]["fetching"] == 0
        assert "/second.html" not in site.requests and len(site.requests) == 2, site.requests
        app.wait_for(lambda: second in app.a11y_names(), "the card shows the address")

        # Both survive a restart.
        app.stop()
        again = app_factory()
        saved = {b["url"]: b for b in links(again)}
        assert set(saved) == {first, second}
        assert saved[first]["title"] == "Designing calm interfaces" and saved[first]["image_exists"]
        assert len(site.requests) == 2, "a restart fetches nothing"
    finally:
        site.close()


# --- Search inside files (E7-08, E7-09) -------------------------------------------------

def tiny_pdf(text: str) -> bytes:
    """A one-page PDF with `text` in Helvetica (uncompressed, for pdftotext)."""
    stream = f"BT /F1 18 Tf 72 720 Td ({text}) Tj ET".encode()
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] "
        b"/Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>",
        b"<< /Length %d >>\nstream\n" % len(stream) + stream + b"\nendstream",
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>",
    ]
    out = b"%PDF-1.4\n"
    offsets = []
    for number, body in enumerate(objects, 1):
        offsets.append(len(out))
        out += b"%d 0 obj\n" % number + body + b"\nendobj\n"
    xref = len(out)
    out += b"xref\n0 %d\n0000000000 65535 f \n" % (len(objects) + 1)
    out += b"".join(b"%010d 00000 n \n" % offset for offset in offsets)
    out += b"trailer\n<< /Size %d /Root 1 0 R >>\nstartxref\n%d\n%%%%EOF\n" % (len(objects) + 1, xref)
    return out


def tiny_docx(paragraph: str) -> bytes:
    import io
    import zipfile
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("[Content_Types].xml",
                   '<?xml version="1.0"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">'
                   '<Override PartName="/word/document.xml" ContentType="application/vnd.openxmlformats-'
                   'officedocument.wordprocessingml.document.main+xml"/></Types>')
        z.writestr("word/document.xml",
                   '<?xml version="1.0"?><w:document xmlns:w="http://schemas.openxmlformats.org/wordprocessingml/2006/main">'
                   f"<w:body><w:p><w:r><w:t>Dear team,</w:t></w:r></w:p><w:p><w:r><w:t>{paragraph}</w:t></w:r></w:p>"
                   "</w:body></w:document>")
    return buffer.getvalue()


def tiny_odt(paragraph: str) -> bytes:
    import io
    import zipfile
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w") as z:
        z.writestr("mimetype", "application/vnd.oasis.opendocument.text")
        z.writestr("content.xml",
                   '<?xml version="1.0"?><office:document-content xmlns:office="urn:oasis:names:tc:opendocument:xmlns:office:1.0" '
                   'xmlns:text="urn:oasis:names:tc:opendocument:xmlns:text:1.0"><office:body><office:text>'
                   f"<text:h>Minutes</text:h><text:p>{paragraph}</text:p></office:text></office:body></office:document-content>")
    return buffer.getvalue()


def search_results(app) -> list[dict]:
    return app.state()["search"]["results"]


def search_contents(app, word: str) -> dict:
    """Opens search with Contents on and returns results by file name."""
    app.press("ctrl+k")
    app.wait_for(lambda: app.state()["search"]["open"], "search")
    if not app.state()["search"]["contents"]:
        app.a11y_click("Search inside files")
        app.wait_for(lambda: app.state()["search"]["contents"], "Contents on")
    app.type_text(word)
    app.wait_for(lambda: app.state()["search"]["ran"] == word, "the query")
    return {r["name"]: r for r in search_results(app)}


def test_E7_08_search_inside_files(session, home, app_factory):
    planted = home / "Documents" / "Planted"
    planted.mkdir()
    (planted / "shopping.txt").write_text("Milk and bread\nRemember the quillfeather order for Monday\n")
    (planted / "report.pdf").write_bytes(tiny_pdf("Quarterly quillfeather figures"))
    (planted / "letter.docx").write_bytes(tiny_docx("The quillfeather contract is signed."))
    (planted / "minutes.odt").write_bytes(tiny_odt("We agreed on the quillfeather budget."))
    app = app_factory()
    status = app.wait_index_idle(timeout=90)
    assert status["contents"]["ok"] >= 4, status

    found = search_contents(app, "quillfeather")
    lines = {
        "shopping.txt": "Remember the quillfeather order for Monday",
        "report.pdf": "Quarterly quillfeather figures",
        "letter.docx": "The quillfeather contract is signed.",
        "minutes.odt": "We agreed on the quillfeather budget.",
    }
    for name, line in lines.items():
        assert name in found, (name, sorted(found))
        assert found[name]["line"] == line, found[name]
        assert found[name]["match"] == "quillfeather"
    # The word is in bold in the line shown.
    assert "Remember the <b>quillfeather</b> order for Monday" in app.state()["search"]["lines_markup"]

    # Off again: only names match, and none of these is named so.
    app.a11y_click("Search inside files")
    app.wait_for(lambda: not app.state()["search"]["contents"], "Contents off")
    app.wait_for(lambda: not any(r["name"] in lines for r in search_results(app)), "name results only")


HANG = "#!/usr/bin/python3\nimport time\nwhile True:\n    time.sleep(1)\n"
NET = """#!/usr/bin/python3
import socket, sys
try:
    socket.create_connection(("127.0.0.1", {port}), timeout=3).close()
    print("netprobe connected")
except OSError as error:
    print("netprobe blocked", type(error).__name__)
"""


def test_E7_09_extraction_is_contained(session, home, app_factory, tmp_path):
    import time
    watcher = Watcher()
    try:
        hang = tmp_path / "hang.py"
        hang.write_text(HANG)
        net = tmp_path / "net.py"
        net.write_text(NET.format(port=watcher.port))
        for helper in (hang, net):
            helper.chmod(0o755)
        planted = home / "Documents" / "Planted"
        planted.mkdir()
        (planted / "stuck.hang").write_text("anything")
        (planted / "probe.net").write_text("anything")
        app = app_factory(extra_env={
            "CAIRN_TEST_EXTRACTORS": f"hang={hang},net={net}",
            "CAIRN_EXTRACT_TIMEOUT_MS": "2000",
        })
        # While the stuck extractor runs, the app answers at once.
        started = time.monotonic()
        app.open_folder(planted)
        app.select("stuck.hang")
        assert time.monotonic() - started < 5, "the window stayed responsive"

        status = app.wait_index_idle(timeout=60)
        assert status["contents"]["timeout"] >= 1, status
        found = search_contents(app, "netprobe")
        assert "probe.net" in found, sorted(found)
        assert found["probe.net"]["line"].startswith("netprobe blocked"), found["probe.net"]
        assert watcher.requests == [], "nothing reached the server from inside the sandbox"
        assert app.running()
    finally:
        watcher.close()


# --- The cairn command (E7-10) ----------------------------------------------------------

def cairn(app, *args: str, program: str | None = None) -> subprocess.CompletedProcess:
    """Runs a `cairn` command in the test session, as a user in a terminal would."""
    binary = program or app.binary
    return subprocess.run([binary, *args], env=app.env(), capture_output=True, text=True, timeout=20)


def test_E7_10_the_cairn_command(app, home):
    app.action("win.workspace-create", s("Taxes 2026"))
    app.wait_for(lambda: app.state()["workspace"]["open"], "the new workspace")
    app.action("win.workspace-close")
    app.wait_for(lambda: not app.state()["workspace"]["open"], "back to the folder")

    done = cairn(app, "open-workspace", "taxes 2026")
    assert done.returncode == 0, done.stderr
    app.wait_for(lambda: app.state()["workspace"]["open"]
                 and app.state()["workspace"]["name"] == "Taxes 2026", "the workspace, by name")

    link = "http://127.0.0.1:9/cli-saved"
    done = cairn(app, "add-link", link)
    assert done.returncode == 0, done.stderr
    app.wait_for(lambda: any(b["url"] == link for b in app.state()["bookmarks"]["all"]), "the saved link")

    invoices = home / "Documents" / "Invoices"
    done = cairn(app, "open", str(invoices))
    assert done.returncode == 0, done.stderr
    app.wait_for(lambda: app.folder()["path"] == str(invoices) and not app.state()["workspace"]["open"],
                 "the folder")

    # cairn-cli, without Qt, does the same.
    cli = str(Path(app.binary).with_name("cairn-cli"))
    done = cairn(app, "free-up-space", program=cli)
    assert done.returncode == 0, done.stderr
    app.wait_for(lambda: app.state()["tools"]["page"] == "clean", "Free Up Space")

    # Mistakes are explained, and change nothing.
    wrong = cairn(app, "add-link")
    assert wrong.returncode == 2 and "add-link needs a link" in wrong.stderr
    assert "open-workspace NAME" in cairn(app, "--help").stdout
    assert app.running()

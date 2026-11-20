"""Fixtures for end-to-end tests (DESIGN.md §10.2).

Every test gets a fresh copy of the fixture home, its own D-Bus session and
headless compositor, and (through `app`) a running app. When a test fails, the
app's state, accessibility tree, a screenshot and all logs are saved under
artifacts/e2e/<test-id>/.
"""

import os
import re
import subprocess
import sys
from pathlib import Path

import pytest

sys.path.insert(0, str(Path(__file__).parent))

from harness import App, Home, Session  # noqa: E402

REPO = Path(__file__).resolve().parents[2]
ARTIFACTS = REPO / "artifacts" / "e2e"
MAKE_FIXTURE = REPO / "docker" / "scripts" / "make-fixture-home"


@pytest.hookimpl(hookwrapper=True, tryfirst=True)
def pytest_runtest_makereport(item, call):
    outcome = yield
    report = outcome.get_result()
    setattr(item, f"rep_{report.when}", report)


def _failed(request) -> bool:
    return any(getattr(request.node, f"rep_{when}", None) is not None
               and getattr(request.node, f"rep_{when}").failed for when in ("setup", "call"))


@pytest.fixture(scope="session")
def fixture_source(tmp_path_factory) -> tuple[Path, dict]:
    """The fixture home, generated once per test run, and the planted times
    of its files (relative path: (atime_ns, mtime_ns)), taken before anything
    reads them."""
    source = tmp_path_factory.mktemp("fixture-source") / "home"
    subprocess.run([sys.executable, "-I", str(MAKE_FIXTURE), str(source)], check=True,
                   capture_output=True)
    times = {}
    for path in source.rglob("*"):
        if path.is_file() and not path.is_symlink():
            st = path.stat()
            times[path.relative_to(source).as_posix()] = (st.st_atime_ns, st.st_mtime_ns)
    return source, times


@pytest.fixture
def home(fixture_source, tmp_path) -> Home:
    """A private copy of the fixture home for this test (sparse files stay sparse).

    Copying reads every file, which moves the source's access times to now,
    and `cp -a` copies those: the planted "not opened for 200 days" would be
    lost. The planted times are put back on the copy."""
    source, times = fixture_source
    dest = tmp_path / "home"
    subprocess.run(["cp", "-a", "--sparse=always", str(source), str(dest)], check=True)
    for rel, stamps in times.items():
        os.utime(dest / rel, ns=stamps, follow_symlinks=False)
    copy = Home(dest)
    _ = copy.manifest  # read before the test can change the marker
    return copy


@pytest.fixture
def session(home, tmp_path):
    started = Session(tmp_path / "session", home.path)
    started.start()
    yield started
    started.stop()


@pytest.fixture
def app_factory(session, home, request):
    """Starts apps on demand: `app_factory()` or `app_factory(wait=False)`."""
    apps: list[App] = []

    def start(wait: bool = True, **kwargs) -> App:
        app = App(session, home, **kwargs)
        apps.append(app)
        return app.launch(wait=wait)

    yield start
    if _failed(request):
        safe = re.sub(r"[^A-Za-z0-9_.-]+", "_", request.node.nodeid)
        for index, app in enumerate(apps):
            app.collect_artifacts(ARTIFACTS / (safe if index == 0 else f"{safe}-{index}"))
    for app in apps:
        app.stop()


@pytest.fixture
def app(app_factory) -> App:
    """The app, started and showing its main window."""
    return app_factory()

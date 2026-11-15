"""Fixtures for end-to-end tests (DESIGN.md §10.2).

Every test gets a fresh copy of the fixture home, its own D-Bus session and
headless compositor, and (through `app`) a running app. When a test fails, the
app's state, accessibility tree, a screenshot and all logs are saved under
artifacts/e2e/<test-id>/.
"""

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
def fixture_source(tmp_path_factory) -> Path:
    """The fixture home, generated once per test run."""
    source = tmp_path_factory.mktemp("fixture-source") / "home"
    subprocess.run([sys.executable, "-I", str(MAKE_FIXTURE), str(source)], check=True,
                   capture_output=True)
    return source


@pytest.fixture
def home(fixture_source, tmp_path) -> Home:
    """A private copy of the fixture home for this test (sparse files stay sparse)."""
    dest = tmp_path / "home"
    subprocess.run(["cp", "-a", "--sparse=always", str(fixture_source), str(dest)], check=True)
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

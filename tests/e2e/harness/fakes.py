"""Stand-ins for system tools (DESIGN.md §10.2).

`FakeTools(folder)` writes small Python programs into `folder` that log every
call and answer with recorded output. Put `folder` first on the app's `PATH`
(`fakes.path(base)`), then read `fakes.calls()` to see exactly what the app
ran. A tool with no answer for its arguments fails like the real one would
for a missing package (exit 1, no output).

Answers are matched on the tool's name and the first arguments:

    fakes.answer("snap", ["list", "--all"], "Name  Version …\\n")
"""

from __future__ import annotations

import json
import os
import sys
from pathlib import Path

SCRIPT = """#!{python} -I
import json, os, sys
from pathlib import Path
here = Path(__file__).resolve().parent
tool, args = Path(sys.argv[0]).name, sys.argv[1:]
with open(here / "calls.jsonl", "a") as log:
    log.write(json.dumps({{"tool": tool, "args": args, "cwd": os.getcwd()}}) + "\\n")
answers = json.loads((here / "answers.json").read_text()).get(tool, [])
for prefix, out, code in answers:
    if args[:len(prefix)] == prefix:
        sys.stdout.write(out)
        sys.exit(code)
sys.exit(1)
"""


class FakeTools:
    def __init__(self, folder: Path) -> None:
        self.folder = folder
        folder.mkdir(parents=True, exist_ok=True)
        self.answers: dict[str, list] = {}
        self._save()

    def _save(self) -> None:
        (self.folder / "answers.json").write_text(json.dumps(self.answers))

    def tool(self, name: str) -> None:
        """Puts `name` on PATH (answering nothing yet)."""
        script = self.folder / name
        if not script.exists():
            script.write_text(SCRIPT.format(python=sys.executable))
            script.chmod(0o755)
        self.answers.setdefault(name, [])
        self._save()

    def answer(self, name: str, prefix: list[str], out: str = "", code: int = 0) -> None:
        self.tool(name)
        self.answers[name].insert(0, [prefix, out, code])
        # The longest matching prefix wins ("list --all" before "list").
        self.answers[name].sort(key=lambda a: -len(a[0]))
        self._save()

    def remove(self, name: str) -> None:
        (self.folder / name).unlink(missing_ok=True)
        self.answers.pop(name, None)
        self._save()

    def calls(self, name: str | None = None) -> list[list[str]]:
        log = self.folder / "calls.jsonl"
        if not log.exists():
            return []
        calls = [json.loads(line) for line in log.read_text().splitlines() if line]
        return [c["args"] for c in calls if name is None or c["tool"] == name] if name else \
            [[c["tool"], *c["args"]] for c in calls]

    def path(self, base: str) -> str:
        return f"{self.folder}{os.pathsep}{base}"

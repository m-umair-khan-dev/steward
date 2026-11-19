"""E2-13 / AGENTS.md rule 10: only cairn-core::ops may change files.

Scans the Rust sources (outside core::ops, tests and build scripts) for calls
that move, copy, rename, trash, delete or create files. A line may carry
"E2-13 allow: <reason>" when it touches the app's own data, never user files.

    python3 tests/tools/check_ops.py [files...]

Exit 0: clean. Exit 1: violations, printed as path:line: text.
"""

import re
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
ALLOWED_DIRS = ("crates/core/src/ops/",)
SKIP = re.compile(r"(^|/)(tests/|build\.rs$|spikes/)")

FORBIDDEN = re.compile(
    r"\bfs::(rename|remove_file|remove_dir|remove_dir_all|copy|create_dir|create_dir_all|write|"
    r"hard_link|set_permissions|soft_link)\b"
    r"|\bFile::create\b|\bOpenOptions::new\b|\bunix::fs::symlink\b"
    r"|\.(trash|delete|copy|move_|make_directory|make_directory_with_parents|set_display_name|"
    r"replace_contents|make_symbolic_link|set_attributes_from_info)(_future|_async)?\s*\("
)


def scan(path: Path) -> list[str]:
    problems = []
    in_tests = False
    for number, line in enumerate(path.read_text(errors="replace").splitlines(), 1):
        if re.match(r"\s*#\[cfg\(test\)\]", line):
            in_tests = True  # test modules sit at the end of a file
        stripped = line.split("//", 1)[0]
        if in_tests or "E2-13 allow:" in line:
            continue
        if FORBIDDEN.search(stripped):
            problems.append(f"{path}:{number}: {line.strip()}")
    return problems


def sources() -> list[Path]:
    found = []
    for path in sorted((REPO / "crates").rglob("*.rs")):
        rel = path.relative_to(REPO).as_posix()
        if rel.startswith(ALLOWED_DIRS) or SKIP.search(rel) or "/target/" in rel:
            continue
        found.append(path)
    return found


def main(argv: list[str]) -> int:
    paths = [Path(a) for a in argv] or sources()
    problems = [p for path in paths for p in scan(path)]
    for problem in problems:
        print(problem)
    if problems:
        print(f"\n{len(problems)} file change(s) outside cairn-core::ops (AGENTS.md rule 10).")
        return 1
    print(f"ops check: {len(paths)} files clean")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))

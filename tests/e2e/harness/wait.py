"""Waiting for conditions. Tests never sleep for a fixed time (AGENTS.md rule 7)."""

import time
from typing import Callable, TypeVar

T = TypeVar("T")


class WaitTimeout(AssertionError):
    pass


def wait_for(condition: Callable[[], T], what: str, timeout: float = 5.0,
             interval: float = 0.05) -> T:
    """Polls `condition` until it returns something truthy, and returns it.

    Exceptions raised by `condition` count as "not yet"; the last one is
    reported if the wait times out."""
    deadline = time.monotonic() + timeout
    last_error: Exception | None = None
    while True:
        try:
            result = condition()
            if result:
                return result
        except Exception as error:  # noqa: BLE001 - reported on timeout
            last_error = error
        if time.monotonic() >= deadline:
            detail = f" (last error: {last_error!r})" if last_error else ""
            raise WaitTimeout(f"timed out after {timeout:.1f}s waiting for {what}{detail}")
        time.sleep(interval)

from __future__ import annotations

import inspect

import pytest
import rerun as rr
from rerun.error_utils import RerunWarning, catch_and_log_exceptions


@catch_and_log_exceptions()
def outer(strict: bool | None = None) -> None:
    """Calls an inner decorated function."""
    inner(3)


@catch_and_log_exceptions()
def inner(count: int) -> None:
    """Calls itself recursively but ultimately raises an error."""
    if count < 0:
        raise ValueError("count must be positive")
    inner(count - 1)


@catch_and_log_exceptions()
def uses_context(strict: bool | None = None) -> None:
    with catch_and_log_exceptions():
        raise ValueError


def get_line_number() -> int:
    """Helper to get a line-number. Make sure our warnings point to the right place."""
    frame = inspect.currentframe().f_back  # type: ignore[union-attr]
    return frame.f_lineno  # type: ignore[union-attr]


def test_stack_tracking() -> None:
    rr.init("test_enable_strict_mode", spawn=False)

    mem = rr.memory_recording()
    with pytest.warns(RerunWarning) as record:
        starting_msgs = mem.num_msgs()
        outer()
        assert record[0].lineno == get_line_number() - 1
        assert record[0].filename == __file__
        assert "count must be positive" in str(record[0].message)
        assert mem.num_msgs() == starting_msgs + 1

    mem = rr.memory_recording()
    with pytest.warns(RerunWarning) as record:
        starting_msgs = mem.num_msgs()
        uses_context()
        assert record[0].lineno == get_line_number() - 1
        assert record[0].filename == __file__
        assert mem.num_msgs() == starting_msgs + 1

    mem = rr.memory_recording()
    with pytest.warns(RerunWarning) as record:
        starting_msgs = mem.num_msgs()
        with catch_and_log_exceptions():
            inner(count=2)
        assert record[0].lineno == get_line_number() - 1
        assert record[0].filename == __file__
        assert "count must be positive" in str(record[0].message)
        assert mem.num_msgs() == starting_msgs + 1

    mem = rr.memory_recording()
    with pytest.warns(RerunWarning) as record:
        starting_msgs = mem.num_msgs()
        with catch_and_log_exceptions():
            raise ValueError
        assert record[0].lineno == get_line_number() - 2
        assert record[0].filename == __file__
        assert mem.num_msgs() == starting_msgs + 1


def test_strict_mode() -> None:
    # We can disable strict on just this function
    with pytest.raises(ValueError):
        outer(strict=True)

    with pytest.raises(ValueError):
        uses_context(strict=True)

    # We can disable strict mode globally
    rr.set_strict_mode(True)
    with pytest.raises(ValueError):
        outer()

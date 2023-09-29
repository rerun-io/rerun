from __future__ import annotations

import inspect
import os
from typing import Any

import pytest
import rerun as rr
from rerun.error_utils import RerunWarning, catch_and_log_exceptions


@catch_and_log_exceptions()
def outer(strict: bool | None = None) -> int:
    """Calls an inner decorated function."""
    inner(3)

    return 42


@catch_and_log_exceptions()
def two_calls(strict: bool | None = None) -> None:
    """Calls an inner decorated function twice."""
    inner(3)
    inner(3)


@catch_and_log_exceptions(context="function context")
def inner(count: int) -> None:
    """Calls itself recursively but ultimately raises an error."""
    if count < 0:
        raise ValueError("some value error")
    inner(count - 1)


@catch_and_log_exceptions()
def uses_context(strict: bool | None = None) -> None:
    """Uses a context manager instead of a function."""
    with catch_and_log_exceptions("inner context"):
        raise ValueError("some value error")


def get_line_number() -> int:
    """Helper to get a line-number. Make sure our warnings point to the right place."""
    frame = inspect.currentframe().f_back  # type: ignore[union-attr]
    return frame.f_lineno  # type: ignore[union-attr]


def expected_warnings(warnings: Any, mem: Any, starting_msgs: int, count: int, expected_line: int) -> None:
    for w in warnings:
        assert w.lineno == expected_line
        assert w.filename == __file__
        assert "some value error" in str(w.message)


def test_stack_tracking() -> None:
    # Force flushing so we can count the messages
    os.environ["RERUN_FLUSH_NUM_ROWS"] = "0"
    rr.init("test_enable_strict_mode", spawn=False)

    mem = rr.memory_recording()
    with pytest.warns(RerunWarning) as warnings:
        starting_msgs = mem.num_msgs()

        assert outer() == 42

        expected_warnings(warnings, mem, starting_msgs, 1, get_line_number() - 2)
        assert "function context" in str(warnings[0].message)

    with pytest.warns(RerunWarning) as warnings:
        starting_msgs = mem.num_msgs()

        two_calls()

        expected_warnings(warnings, mem, starting_msgs, 2, get_line_number() - 2)

    with pytest.warns(RerunWarning) as warnings:
        starting_msgs = mem.num_msgs()

        uses_context()

        expected_warnings(warnings, mem, starting_msgs, 1, get_line_number() - 2)

    with pytest.warns(RerunWarning) as warnings:
        starting_msgs = mem.num_msgs()

        with catch_and_log_exceptions():
            uses_context()

        expected_warnings(warnings, mem, starting_msgs, 1, get_line_number() - 3)
        assert "inner context" in str(warnings[0].message)

    with pytest.warns(RerunWarning) as warnings:
        starting_msgs = mem.num_msgs()

        with catch_and_log_exceptions("some context"):
            raise ValueError("some value error")

        expected_warnings(warnings, mem, starting_msgs, 1, get_line_number() - 3)
        assert "some context" in str(warnings[0].message)


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
    # Clear the global strict mode again
    rr.set_strict_mode(False)


def test_bad_components() -> None:
    with pytest.warns(RerunWarning) as warnings:
        points = rr.Points3D(positions=[1, 2, 3], colors="RED")
        assert len(warnings) == 1
        assert len(points.positions) == 1
        assert len(points.colors) == 0  # type: ignore[arg-type]

    rr.set_strict_mode(True)
    with pytest.raises(ValueError):
        points = rr.Points3D(positions=[1, 2, 3], colors="RED")

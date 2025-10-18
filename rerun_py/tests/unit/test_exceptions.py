from __future__ import annotations

import inspect
import os
from typing import Any

import pytest
import rerun as rr
from rerun.error_utils import RerunWarning, catch_and_log_exceptions


@catch_and_log_exceptions()
def outer(strict: bool | None = None) -> int:  # noqa: ARG001 - `strict` handled by `@catch_and_log_exceptions`
    """Calls an inner decorated function."""
    inner(3)

    return 42


@catch_and_log_exceptions()
def two_calls(strict: bool | None = None) -> None:  # noqa: ARG001 - `strict` handled by `@catch_and_log_exceptions`
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
def uses_context(strict: bool | None = None) -> None:  # noqa: ARG001 - `strict` handled by `@catch_and_log_exceptions`
    """Uses a context manager instead of a function."""
    with catch_and_log_exceptions("inner context"):
        raise ValueError("some value error")


def get_line_number() -> int:
    """Helper to get a line-number. Make sure our warnings point to the right place."""
    frame = inspect.currentframe().f_back  # type: ignore[union-attr]
    return frame.f_lineno  # type: ignore[union-attr]


def expected_warnings(warnings: Any, mem: Any, starting_msgs: int, count: int, expected_line: int) -> None:
    for w in warnings:
        assert w.lineno == expected_line, f"mem: {mem}, starting_msgs: {starting_msgs}, count: {count}"
        assert w.filename == __file__, f"mem: {mem}, starting_msgs: {starting_msgs}, count: {count}"
        assert "some value error" in str(w.message), f"mem: {mem}, starting_msgs: {starting_msgs}, count: {count}"


def test_stack_tracking() -> None:
    # Force flushing so we can count the messages
    os.environ["RERUN_FLUSH_NUM_ROWS"] = "0"
    rr.init("rerun_example_strict_mode", strict=False, spawn=False)

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

        value = 0
        with catch_and_log_exceptions(depth_to_user_code=0):
            uses_context()
            value = 42

        expected_line = get_line_number() - 4  # the first line of the context block
        expected_warnings(warnings, mem, starting_msgs, 1, expected_line)
        # value is changed because uses_context its own exception internally
        assert value == 42
        assert "inner context" in str(warnings[0].message)

    with pytest.warns(RerunWarning) as warnings:
        starting_msgs = mem.num_msgs()

        value = 0
        with catch_and_log_exceptions("some context", depth_to_user_code=0):
            raise ValueError("some value error")
            value = 42

        expected_line = get_line_number() - 4  # the open of the context manager
        expected_warnings(warnings, mem, starting_msgs, 1, expected_line)
        # value wasn't changed because an exception was raised
        assert value == 0
        assert "some context" in str(warnings[0].message)


def test_strict_mode() -> None:
    rr.set_strict_mode(False)

    # Confirm strict mode is off
    with pytest.warns(RerunWarning):
        assert outer() == 42

    # We can enable strict on just this function
    with pytest.raises(ValueError):
        outer(strict=True)

    with pytest.raises(ValueError):
        uses_context(strict=True)

    # We can enable strict mode globally
    rr.set_strict_mode(True)
    with pytest.raises(ValueError):
        outer()

    # Clear the global strict mode again
    rr.set_strict_mode(False)


def test_bad_components() -> None:
    with pytest.warns(RerunWarning) as warnings:
        points = rr.Points3D(positions=[1, 2, 3], colors="RED")
        assert len(warnings) == 1
        assert points.positions is not None and len(points.positions) == 1
        assert points.colors is not None and len(points.colors) == 0

    rr.set_strict_mode(True)
    with pytest.raises(ValueError):
        points = rr.Points3D(positions=[1, 2, 3], colors="RED")

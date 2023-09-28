from __future__ import annotations

import inspect

import pytest
import rerun as rr
from rerun.error_utils import RerunWarning, catch_and_log_exceptions


@catch_and_log_exceptions
def outer() -> None:
    inner(3)


@catch_and_log_exceptions
def inner(count: int) -> None:
    if count < 0:
        raise ValueError("count must be positive")
    inner(count - 1)


def get_line_number() -> int:
    frame = inspect.currentframe().f_back  # type: ignore[union-attr]
    return frame.f_lineno  # type: ignore[union-attr]


def test_enable_strict_mode() -> None:
    rr.init("test_enable_strict_mode", spawn=False)
    mem = rr.memory_recording()
    with pytest.warns(RerunWarning) as record:
        starting_msgs = mem.num_msgs()
        outer()
        assert record[0].lineno == get_line_number() - 1
        assert record[0].filename == __file__
        assert "count must be positive" in str(record[0].message)
        assert mem.num_msgs() == starting_msgs + 1

    rr.set_strict_mode(True)
    with pytest.raises(ValueError):
        outer()

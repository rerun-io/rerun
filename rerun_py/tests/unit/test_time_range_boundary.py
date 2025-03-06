from __future__ import annotations

import pytest
import rerun as rr


def test_time_range_boundary_failure_cases() -> None:
    # Too many arguments for absolute.
    with pytest.raises(ValueError):
        rr.TimeRangeBoundary.absolute(rr.TimeInt(seq=0), seq=123)  # type: ignore[call-overload]
    with pytest.raises(ValueError):
        rr.TimeRangeBoundary.absolute(rr.TimeInt(seq=0), seconds=123.0)  # type: ignore[call-overload]
    with pytest.raises(ValueError):
        rr.TimeRangeBoundary.absolute(rr.TimeInt(seq=0), nanos=123)  # type: ignore[call-overload]
    with pytest.raises(ValueError):
        rr.TimeRangeBoundary.absolute(seq=123, seconds=123.0)  # type: ignore[call-overload]
    with pytest.raises(ValueError):
        rr.TimeRangeBoundary.absolute(seq=123, nanos=123)  # type: ignore[call-overload]
    with pytest.raises(ValueError):
        rr.TimeRangeBoundary.absolute(seconds=123, seq=123)  # type: ignore[call-overload]
    with pytest.raises(ValueError):
        rr.TimeRangeBoundary.absolute(seconds=123, nanos=123)  # type: ignore[call-overload]
    with pytest.raises(ValueError):
        rr.TimeRangeBoundary.absolute(nanos=123, seq=123)  # type: ignore[call-overload]
    with pytest.raises(ValueError):
        rr.TimeRangeBoundary.absolute(nanos=123, seconds=123.0)  # type: ignore[call-overload]

    # No argument for absolute.
    with pytest.raises(ValueError):
        rr.TimeRangeBoundary.absolute()  # type: ignore[call-overload]

    # Too many arguments for cursor_relative.
    with pytest.raises(ValueError):
        rr.TimeRangeBoundary.cursor_relative(rr.TimeInt(seq=0), seq=123)  # type: ignore[call-overload]
    with pytest.raises(ValueError):
        rr.TimeRangeBoundary.cursor_relative(rr.TimeInt(seq=0), seconds=123.0)  # type: ignore[call-overload]
    with pytest.raises(ValueError):
        rr.TimeRangeBoundary.cursor_relative(rr.TimeInt(seq=0), nanos=123)  # type: ignore[call-overload]
    with pytest.raises(ValueError):
        rr.TimeRangeBoundary.cursor_relative(seq=123, seconds=123.0)  # type: ignore[call-overload]
    with pytest.raises(ValueError):
        rr.TimeRangeBoundary.cursor_relative(seq=123, nanos=123)  # type: ignore[call-overload]
    with pytest.raises(ValueError):
        rr.TimeRangeBoundary.cursor_relative(seconds=123, seq=123)  # type: ignore[call-overload]
    with pytest.raises(ValueError):
        rr.TimeRangeBoundary.cursor_relative(seconds=123, nanos=123)  # type: ignore[call-overload]
    with pytest.raises(ValueError):
        rr.TimeRangeBoundary.cursor_relative(nanos=123, seq=123)  # type: ignore[call-overload]
    with pytest.raises(ValueError):
        rr.TimeRangeBoundary.cursor_relative(nanos=123, seconds=123.0)  # type: ignore[call-overload]


def test_time_range_boundary() -> None:
    # Test infinite.
    assert rr.TimeRangeBoundary.infinite().kind == "infinite"
    assert rr.TimeRangeBoundary.infinite().inner is None

    # Test absolute.
    assert rr.TimeRangeBoundary.absolute(seq=123).kind == "absolute"
    assert rr.TimeRangeBoundary.absolute(seq=123).inner == rr.TimeInt(seq=123)
    assert rr.TimeRangeBoundary.absolute(seq=123).inner == rr.TimeInt(nanos=123)
    assert rr.TimeRangeBoundary.absolute(seconds=1.0).inner == rr.TimeInt(nanos=int(1e9))

    # Test cursor_relative.
    assert rr.TimeRangeBoundary.cursor_relative(seq=123).kind == "cursor_relative"
    assert rr.TimeRangeBoundary.cursor_relative(seq=123).inner == rr.TimeInt(seq=123)
    assert rr.TimeRangeBoundary.cursor_relative(seq=123).inner == rr.TimeInt(nanos=123)
    assert rr.TimeRangeBoundary.cursor_relative(nanos=123).inner == rr.TimeInt(nanos=123)
    assert rr.TimeRangeBoundary.cursor_relative(seconds=1.0).inner == rr.TimeInt(nanos=int(1e9))
    assert rr.TimeRangeBoundary.cursor_relative().kind == "cursor_relative"
    assert rr.TimeRangeBoundary.cursor_relative().inner == rr.TimeInt(seq=0)

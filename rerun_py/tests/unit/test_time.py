from __future__ import annotations

from datetime import datetime, timedelta, timezone
from typing import Any

import numpy as np
import pyarrow as pa
import pytest
from rerun.time import to_nanos, to_nanos_since_epoch

VALID_TO_NANOS_CASES = [
    (0, 0),
    (2, 2_000_000_000),
    (np.int8(3), 3_000_000_000),
    (np.int16(4), 4_000_000_000),
    (np.int32(5), 5_000_000_000),
    (np.int64(6), 6_000_000_000),
    (0.75, 750_000_000),
    (np.float64(3.6), 3_600_000_000),
    (timedelta(seconds=2, microseconds=500_000), 2_500_000_000),
    (np.timedelta64(4, "s"), 4_000_000_000),
    (np.timedelta64(2500, "ms"), 2_500_000_000),
    (np.timedelta64(25, "ns"), 25),
    (-3, -3_000_000_000),
    (np.int64(-2), -2_000_000_000),
    (np.float64(-1.5), -1_500_000_000),
    (timedelta(seconds=-4, microseconds=-500_000), -4_500_000_000),
    (np.timedelta64(-7, "s"), -7_000_000_000),
]


@pytest.mark.parametrize("duration,expected", VALID_TO_NANOS_CASES)
def test_to_nanos_valid(duration: int | float | timedelta | np.timedelta64, expected: int) -> None:
    assert to_nanos(duration) == expected


INVALID_TO_NANOS_CASES = [
    "invalid",
    None,
    [1, 2, 3],
    3 + 4j,
]


@pytest.mark.parametrize("duration", INVALID_TO_NANOS_CASES)
def test_to_nanos_invalid(duration: Any) -> None:
    with pytest.raises(TypeError):
        to_nanos(duration)


class MockPandasTimestamp(datetime):
    nanoseconds: int = 0

    def __new__(cls, *args: Any, nanoseconds: int = 0, **kwargs: dict[str, Any]) -> MockPandasTimestamp:
        instance = super().__new__(cls, *args, **kwargs)  # type: ignore[arg-type]
        object.__setattr__(instance, "nanoseconds", nanoseconds)
        return instance

    def to_datetime64(self) -> np.datetime64:
        return np.datetime64(self.replace(microsecond=0).isoformat() + f".{self.nanoseconds:09d}", "ns")


VALID_TO_NANOS_SINCE_EPOCH_CASES = [
    (0, 0),
    (10, 10_000_000_000),
    (np.int8(127), 127_000_000_000),
    (np.int32(50), 50_000_000_000),
    (np.int64(1223334444), 1_223_334_444_000_000_000),
    (1.5, 1_500_000_000),
    (np.float64(2.7), 2_700_000_000),
    (datetime(1970, 1, 1), 0),
    (datetime(1970, 1, 1, tzinfo=timezone.utc), 0),
    (datetime(1970, 1, 1, 0, 0, 1), 1_000_000_000),
    (np.datetime64("1970-01-01T00:00:00"), 0),
    (np.datetime64("1970-01-01T00:00:01"), 1_000_000_000),
    (np.datetime64("1970-01-01T00:00:00.000000000"), 0),
    (datetime(1969, 12, 31, 23, 59, 59, tzinfo=timezone.utc), -1_000_000_000),
    (np.datetime64("1969-12-31T23:59:59"), -1_000_000_000),
    (datetime(2050, 1, 1, 0, 0, 0, tzinfo=timezone.utc), 2524608000000000000),
    (np.datetime64("2050-01-01T00:00:00"), 2524608000000000000),
    (np.datetime64("2000-01-01T00:00:00.123456789"), 946684800123456789),
    (MockPandasTimestamp(2000, 1, 1, 0, 0, 0, 123456, nanoseconds=123456789), 946684800123456789),
    (pa.scalar(np.datetime64("2000-01-01T00:00:00.123456789"), type=pa.timestamp("ns")), 946684800123456789),
]


@pytest.mark.parametrize("timestamp,expected", VALID_TO_NANOS_SINCE_EPOCH_CASES)
def test_to_nanos_since_epoch_valid(timestamp: int | float | datetime | np.datetime64, expected: int) -> None:
    assert to_nanos_since_epoch(timestamp) == expected


INVALID_TO_NANOS_SINCE_EPOCH_CASES = [
    "invalid",
    None,
    # Not enough precision for nanosecond scale
    np.float16(14.234),
    np.float32(211.623),
    [1, 2, 3],
]


@pytest.mark.parametrize("timestamp", INVALID_TO_NANOS_SINCE_EPOCH_CASES)
def test_to_nanos_since_epoch_invalid(timestamp: Any) -> None:
    with pytest.raises(TypeError):
        to_nanos_since_epoch(timestamp)

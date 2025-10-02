from __future__ import annotations

from datetime import datetime, timedelta
from typing import TYPE_CHECKING

import numpy as np
import pyarrow as pa
import pytest
import rerun as rr

if TYPE_CHECKING:
    from collections.abc import Iterable

VALID_SEQUENCE_CASES = [
    ([0, 1, 2, 3], pa.array([0, 1, 2, 3], type=pa.int64())),
    ([-1, 0, 1], pa.array([-1, 0, 1], type=pa.int64())),
    (np.array([10, 20, 30]), pa.array([10, 20, 30], type=pa.int64())),
]


@pytest.mark.parametrize("sequence,expected", VALID_SEQUENCE_CASES)
def test_sequence_column(sequence: Iterable[int], expected: pa.Array) -> None:
    column = rr.TimeColumn("sequence", sequence=sequence)
    assert column.as_arrow_array() == expected


VALID_DURATION_CASES = [
    ([0, 1, 2, 3], pa.array([0, 1_000_000_000, 2_000_000_000, 3_000_000_000], type=pa.duration("ns"))),
    (
        np.arange(10, 15, 1.0),
        pa.array(
            [10_000_000_000, 11_000_000_000, 12_000_000_000, 13_000_000_000, 14_000_000_000],
            type=pa.duration("ns"),
        ),
    ),
    ([0.0, 1.5, 2.25, 3.0], pa.array([0, 1_500_000_000, 2_250_000_000, 3_000_000_000], type=pa.duration("ns"))),
    (
        [
            timedelta(seconds=0),
            timedelta(seconds=1),
            timedelta(seconds=1, microseconds=500000),
            timedelta(seconds=2, microseconds=250000),
        ],
        pa.array([0, 1_000_000_000, 1_500_000_000, 2_250_000_000], type=pa.duration("ns")),
    ),
    (
        [np.timedelta64(0, "s"), np.timedelta64(1, "s"), np.timedelta64(1500, "ms"), np.timedelta64(2250, "ms")],
        pa.array([0, 1_000_000_000, 1_500_000_000, 2_250_000_000], type=pa.duration("ns")),
    ),
    ([-1, 0, 1], pa.array([-1_000_000_000, 0, 1_000_000_000], type=pa.duration("ns"))),
    (
        np.array([np.timedelta64(0, "s"), np.timedelta64(1, "s"), np.timedelta64(1500, "ms")]),
        pa.array([0, 1_000_000_000, 1_500_000_000], type=pa.duration("ns")),
    ),
]


@pytest.mark.parametrize("duration,expected", VALID_DURATION_CASES)
def test_duration_column(
    duration: Iterable[int] | Iterable[float] | Iterable[timedelta] | Iterable[np.timedelta64], expected: pa.Array
) -> None:
    column = rr.TimeColumn("duration", duration=duration)

    assert column.as_arrow_array() == expected


VALID_TIMESTAMP_CASES = [
    (
        [0, 1, 2, 3],
        pa.array(
            [
                np.datetime64("1970-01-01T00:00:00", "ns"),
                np.datetime64("1970-01-01T00:00:01", "ns"),
                np.datetime64("1970-01-01T00:00:02", "ns"),
                np.datetime64("1970-01-01T00:00:03", "ns"),
            ],
            type=pa.timestamp("ns"),
        ),
    ),
    (
        [0.0, 1.5, 2.25, 3.0],
        pa.array(
            [
                np.datetime64("1970-01-01T00:00:00", "ns"),
                np.datetime64("1970-01-01T00:00:01.50", "ns"),
                np.datetime64("1970-01-01T00:00:02.25", "ns"),
                np.datetime64("1970-01-01T00:00:03", "ns"),
            ],
            type=pa.timestamp("ns"),
        ),
    ),
    (
        np.array([0, 1, 2, 3]),
        pa.array(
            [
                np.datetime64("1970-01-01T00:00:00", "ns"),
                np.datetime64("1970-01-01T00:00:01", "ns"),
                np.datetime64("1970-01-01T00:00:02", "ns"),
                np.datetime64("1970-01-01T00:00:03", "ns"),
            ],
            type=pa.timestamp("ns"),
        ),
    ),
    (
        np.array([0.0, 1.5, 2.25, 3.0]),
        pa.array(
            [
                np.datetime64("1970-01-01T00:00:00", "ns"),
                np.datetime64("1970-01-01T00:00:01.50", "ns"),
                np.datetime64("1970-01-01T00:00:02.25", "ns"),
                np.datetime64("1970-01-01T00:00:03", "ns"),
            ],
            type=pa.timestamp("ns"),
        ),
    ),
    (
        [datetime(2020, 1, 1, 0, 0, 0), datetime(2020, 1, 1, 0, 0, 1)],
        pa.array(
            [
                np.datetime64("2020-01-01T00:00:00", "ns"),
                np.datetime64("2020-01-01T00:00:01", "ns"),
            ],
            type=pa.timestamp("ns"),
        ),
    ),
    (
        np.array([np.datetime64("2020-01-01T00:00:00"), np.datetime64("2020-01-01T00:00:01")]),
        pa.array(
            [np.datetime64("2020-01-01T00:00:00", "ns"), np.datetime64("2020-01-01T00:00:01", "ns")],
            type=pa.timestamp("ns"),
        ),
    ),
]


@pytest.mark.parametrize("timestamp,expected", VALID_TIMESTAMP_CASES)
def test_timestamp_column(
    timestamp: Iterable[int] | Iterable[float] | Iterable[datetime] | Iterable[np.datetime64], expected: pa.Array
) -> None:
    column = rr.TimeColumn("timestamp", timestamp=timestamp)
    assert column.as_arrow_array() == expected

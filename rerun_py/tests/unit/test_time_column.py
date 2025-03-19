from __future__ import annotations

from datetime import datetime, timedelta, timezone
from typing import Any, Iterable

import numpy as np
import pytest

import pyarrow as pa
import rerun as rr

VALID_DURATION_CASES = [
    ([0, 1, 2, 3], pa.array([0, 1_000_000_000, 2_000_000_000, 3_000_000_000], type=pa.duration("ns"))),
]


@pytest.mark.parametrize("duration,expected", VALID_DURATION_CASES)
def test_time_column(
    duration: Iterable[int] | Iterable[float] | Iterable[timedelta] | Iterable[np.timedelta64], expected: pa.Array
) -> None:
    column = rr.TimeColumn("duration", duration=duration)

    assert column.as_arrow_array() == expected

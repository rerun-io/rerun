"""
End-to-end coverage for `SampleIndex.build` across the three timeline kinds.

Uses the shared `readonly_test_dataset` fixture, which exposes timelines of
each kind on the same recording:

- `log_tick` (sequence / Int64)
- `log_time` (timestamp / Timestamp(ns))
- `time_2`   (duration / Duration(ns))
"""

from __future__ import annotations

from typing import TYPE_CHECKING

import numpy as np
import pytest
from rerun.experimental.dataloader import DataSource, FixedRateSampling, SampleIndex

if TYPE_CHECKING:
    from rerun.catalog import DatasetEntry


def test_build_integer_timeline(readonly_test_dataset: DatasetEntry) -> None:
    sample_index = SampleIndex.build(DataSource(dataset=readonly_test_dataset), index="log_tick", fields={})

    assert sample_index.ns_dtype is None
    assert sample_index.ns_per_sample is None
    assert sample_index.total_samples > 0

    _segment, value = sample_index.global_to_local(0)
    assert isinstance(value, int)


def test_build_timestamp_timeline(readonly_test_dataset: DatasetEntry) -> None:
    sample_index = SampleIndex.build(
        DataSource(dataset=readonly_test_dataset),
        index="log_time",
        fields={},
        timeline_sampling=FixedRateSampling(rate_hz=1000.0),
    )

    assert sample_index.ns_dtype == "datetime64[ns]"
    assert sample_index.is_timestamp
    assert sample_index.ns_per_sample == 1_000_000  # 1 ms
    assert sample_index.total_samples > 0

    _segment, value = sample_index.global_to_local(0)
    assert isinstance(value, np.datetime64)


def test_build_duration_timeline(readonly_test_dataset: DatasetEntry) -> None:
    """Regression: duration timelines were routed through the integer path and crashed on `int(Timedelta)`."""
    sample_index = SampleIndex.build(
        DataSource(dataset=readonly_test_dataset),
        index="time_2",
        fields={},
        timeline_sampling=FixedRateSampling(rate_hz=1.0),
    )

    assert sample_index.ns_dtype == "timedelta64[ns]"
    assert sample_index.is_duration
    assert sample_index.ns_per_sample == 1_000_000_000  # 1 s
    assert sample_index.total_samples > 0

    _segment, value = sample_index.global_to_local(0)
    assert isinstance(value, np.timedelta64)


def test_build_duration_without_rate_raises(readonly_test_dataset: DatasetEntry) -> None:
    with pytest.raises(TypeError, match="duration timeline"):
        SampleIndex.build(DataSource(dataset=readonly_test_dataset), index="time_2", fields={})

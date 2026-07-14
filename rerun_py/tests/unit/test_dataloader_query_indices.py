"""Tests for `_build_query_indices` in `rerun.experimental.dataloader._utils`."""

from __future__ import annotations

import numpy as np
import pyarrow as pa
import pytest
from rerun.experimental.dataloader._sample_index import SampleIndex, SegmentMetadata
from rerun.experimental.dataloader._utils import Target, _build_query_indices


def _segment(segment_id: str, index_start: int, num_samples: int, ns_per_sample: int) -> SegmentMetadata:
    return SegmentMetadata(
        segment_id=segment_id,
        index_start=index_start,
        index_end=index_start + (num_samples - 1) * ns_per_sample,
        num_samples=num_samples,
    )


def _targets(sample_index: SampleIndex, count: int) -> list[Target]:
    """`Target`s for the first `count` global indices, with no keyframe anchors."""
    located = (sample_index.global_to_local(i) for i in range(count))
    return [Target(segment=segment, index_value=value, anchors={}) for segment, value in located]


@pytest.mark.parametrize(
    ("ns_dtype", "expected_arrow_type"),
    [
        ("datetime64[ns]", pa.timestamp("ns")),
        ("timedelta64[ns]", pa.duration("ns")),
    ],
)
def test_build_query_indices_temporal_returns_pyarrow(ns_dtype: str, expected_arrow_type: pa.DataType) -> None:
    """
    Temporal timelines must hand values to the Rust binding as pyarrow arrays.

    `IndexValuesLike::extract_bound` accepts `datetime64` ndarrays but not
    `timedelta64`, so the dataloader routes both temporal kinds through
    `pa.array(…, timestamp("ns") | duration("ns"))` instead.
    """
    ns_per_sample = 10_000_000  # 100 Hz
    segment = _segment("seg-a", index_start=0, num_samples=3, ns_per_sample=ns_per_sample)
    sample_index = SampleIndex([segment], ns_per_sample=ns_per_sample, ns_dtype=ns_dtype)

    targets = _targets(sample_index, 3)
    result = _build_query_indices(targets, fields={}, decoders={}, sample_index=sample_index)

    assert set(result.keys()) == {"seg-a"}
    values = result["seg-a"]
    assert isinstance(values, pa.Array)
    assert values.type == expected_arrow_type
    assert values.cast(pa.int64()).to_pylist() == [0, ns_per_sample, 2 * ns_per_sample]


def test_build_query_indices_integer_returns_ndarray() -> None:
    """Integer timelines keep the int64 ndarray path."""
    segment = SegmentMetadata(segment_id="seg-a", index_start=10, index_end=12, num_samples=3)
    sample_index = SampleIndex([segment])

    targets = _targets(sample_index, 3)
    result = _build_query_indices(targets, fields={}, decoders={}, sample_index=sample_index)

    values = result["seg-a"]
    assert isinstance(values, np.ndarray)
    assert values.dtype == np.int64
    assert values.tolist() == [10, 11, 12]

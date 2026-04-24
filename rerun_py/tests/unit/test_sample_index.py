"""Tests for `SampleIndex.global_to_local` in `rerun.experimental.dataloader._sample_index`."""

from __future__ import annotations

import numpy as np
import pytest
from rerun.experimental.dataloader._sample_index import SampleIndex, SegmentMetadata


def _integer_segment(segment_id: str, index_start: int, index_end: int) -> SegmentMetadata:
    return SegmentMetadata(
        segment_id=segment_id,
        index_start=index_start,
        index_end=index_end,
        num_samples=index_end - index_start + 1,
    )


def _fixed_rate_segment(
    segment_id: str,
    index_start: int,
    num_samples: int,
    ns_per_sample: int,
) -> SegmentMetadata:
    return SegmentMetadata(
        segment_id=segment_id,
        index_start=index_start,
        index_end=index_start + (num_samples - 1) * ns_per_sample,
        num_samples=num_samples,
    )


def test_global_to_local_integer_single_segment() -> None:
    seg = _integer_segment("seg-a", index_start=10, index_end=14)
    sample_index = SampleIndex([seg])

    for pos in range(seg.num_samples):
        resolved_seg, value = sample_index.global_to_local(pos)
        assert resolved_seg is seg
        assert value == 10 + pos
        assert isinstance(value, int)


def test_global_to_local_integer_multiple_segments() -> None:
    seg_a = _integer_segment("seg-a", index_start=0, index_end=2)  # 3 samples: 0,1,2
    seg_b = _integer_segment("seg-b", index_start=100, index_end=101)  # 2 samples: 100,101
    seg_c = _integer_segment("seg-c", index_start=50, index_end=50)  # 1 sample: 50
    sample_index = SampleIndex([seg_a, seg_b, seg_c])

    assert sample_index.total_samples == 6

    expected = [
        (seg_a, 0),
        (seg_a, 1),
        (seg_a, 2),
        (seg_b, 100),
        (seg_b, 101),
        (seg_c, 50),
    ]
    for global_idx, (expected_seg, expected_value) in enumerate(expected):
        resolved_seg, value = sample_index.global_to_local(global_idx)
        assert resolved_seg is expected_seg
        assert value == expected_value
        assert isinstance(value, int)


def test_global_to_local_fixed_rate_timestamp() -> None:
    ns_per_sample = 10_000_000  # 100 Hz
    seg_a = _fixed_rate_segment("seg-a", index_start=1_000_000_000, num_samples=3, ns_per_sample=ns_per_sample)
    seg_b = _fixed_rate_segment("seg-b", index_start=2_000_000_000, num_samples=2, ns_per_sample=ns_per_sample)
    sample_index = SampleIndex([seg_a, seg_b], ns_per_sample=ns_per_sample, is_timestamp=True)

    assert sample_index.total_samples == 5

    expected = [
        (seg_a, np.datetime64(1_000_000_000, "ns")),
        (seg_a, np.datetime64(1_010_000_000, "ns")),
        (seg_a, np.datetime64(1_020_000_000, "ns")),
        (seg_b, np.datetime64(2_000_000_000, "ns")),
        (seg_b, np.datetime64(2_010_000_000, "ns")),
    ]
    for global_idx, (expected_seg, expected_value) in enumerate(expected):
        resolved_seg, value = sample_index.global_to_local(global_idx)
        assert resolved_seg is expected_seg
        assert isinstance(value, np.datetime64)
        assert value == expected_value


@pytest.mark.parametrize("bad_idx", [-1, 6, 100])
def test_global_to_local_out_of_range_raises(bad_idx: int) -> None:
    seg_a = _integer_segment("seg-a", index_start=0, index_end=2)
    seg_b = _integer_segment("seg-b", index_start=10, index_end=12)
    sample_index = SampleIndex([seg_a, seg_b])

    assert sample_index.total_samples == 6
    with pytest.raises(IndexError):
        sample_index.global_to_local(bad_idx)


def test_global_to_local_empty_raises() -> None:
    sample_index = SampleIndex([])
    assert sample_index.total_samples == 0
    with pytest.raises(IndexError):
        sample_index.global_to_local(0)


def test_global_to_local_segment_boundaries() -> None:
    # Three segments with distinct sizes to catch off-by-one at boundaries.
    seg_a = _integer_segment("seg-a", index_start=0, index_end=0)  # 1 sample
    seg_b = _integer_segment("seg-b", index_start=10, index_end=13)  # 4 samples
    seg_c = _integer_segment("seg-c", index_start=100, index_end=101)  # 2 samples
    sample_index = SampleIndex([seg_a, seg_b, seg_c])

    # Last index of each segment.
    last_a_seg, last_a_value = sample_index.global_to_local(0)
    assert last_a_seg is seg_a
    assert last_a_value == 0

    last_b_seg, last_b_value = sample_index.global_to_local(4)
    assert last_b_seg is seg_b
    assert last_b_value == 13

    last_c_seg, last_c_value = sample_index.global_to_local(6)
    assert last_c_seg is seg_c
    assert last_c_value == 101

    # First index of each non-initial segment.
    first_b_seg, first_b_value = sample_index.global_to_local(1)
    assert first_b_seg is seg_b
    assert first_b_value == 10

    first_c_seg, first_c_value = sample_index.global_to_local(5)
    assert first_c_seg is seg_c
    assert first_c_value == 100

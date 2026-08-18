"""Tests for the query-shaping helpers `_build_query_indices` and `_build_query_plans`."""

from __future__ import annotations

import numpy as np
import pyarrow as pa
import pytest
from rerun.experimental.dataloader import Field
from rerun.experimental.dataloader._sample_index import SampleIndex, SegmentMetadata
from rerun.experimental.dataloader._utils import (
    FieldFetchRequest,
    Target,
    _build_query_indices,
    _build_query_plans,
    _build_target,
)
from rerun.experimental.dataloader.decoders import ImageDecoder, NumericDecoder, VideoFrameDecoder


def _segment(segment_id: str, index_start: int, num_samples: int, ns_per_sample: int) -> SegmentMetadata:
    return SegmentMetadata(
        segment_id=segment_id,
        index_start=index_start,
        index_end=index_start + (num_samples - 1) * ns_per_sample,
        num_samples=num_samples,
    )


def _targets(sample_index: SampleIndex, count: int, fields: dict[str, Field] | None = None) -> list[Target]:
    """`Target`s for the first `count` global indices, with no keyframe anchors."""
    fields = fields or {"x": Field(path="/x", decode=NumericDecoder())}
    decoders = {key: field.decode for key, field in fields.items()}
    located = (sample_index.global_to_local(i) for i in range(count))
    return [
        _build_target(
            sample_position=position,
            segment=segment,
            index_value=value,
            fields=fields,
            decoders=decoders,
            sample_index=sample_index,
        )
        for position, (segment, value) in enumerate(located)
    ]


def _requests(targets: list[Target]) -> dict[str, list[FieldFetchRequest]]:
    return {key: [target.fetch_requests[key] for target in targets] for key in targets[0].fetch_requests}


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
    result = _build_query_indices(_requests(targets), sample_index=sample_index)

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
    result = _build_query_indices(_requests(targets), sample_index=sample_index)

    values = result["seg-a"]
    assert isinstance(values, np.ndarray)
    assert values.dtype == np.int64
    assert values.tolist() == [10, 11, 12]


def _grouping(fields: dict[str, Field]) -> list[list[str]]:
    """The field keys of each query plan, sorted (inner and outer) for stable comparison."""
    sample_index = SampleIndex([SegmentMetadata(segment_id="seg", index_start=0, index_end=0, num_samples=1)])
    plans = _build_query_plans(_targets(sample_index, 1, fields), fields, sample_index=sample_index)
    return sorted(sorted(plan.fields) for plan in plans)


def test_query_plan_contains_resolved_indices() -> None:
    sample_index = SampleIndex([SegmentMetadata(segment_id="seg-a", index_start=10, index_end=20, num_samples=11)])
    fields = {
        "action": Field(path="/action:Scalars:scalars", decode=NumericDecoder(), window=(0, 2)),
    }
    targets = _targets(sample_index, 1, fields)

    plans = _build_query_plans(
        targets,
        fields,
        sample_index=sample_index,
    )

    assert len(plans) == 1
    values = plans[0].query_indices["seg-a"]
    assert isinstance(values, np.ndarray)
    assert values.tolist() == [10, 11, 12]
    assert plans[0].fetch_requests["action"] == [targets[0].fetch_requests["action"]]


def test_windowed_field_does_not_share_group_with_unwindowed() -> None:
    """A shared query would ship the unwindowed image at every index value of the action's window."""
    fields = {
        "image": Field(path="/camera:EncodedImage:blob", decode=ImageDecoder()),
        "action": Field(path="/action:Scalars:scalars", decode=NumericDecoder(), window=(0, 19)),
    }
    assert _grouping(fields) == [["action"], ["image"]]


def test_same_window_fields_share_a_group() -> None:
    fields = {
        "action": Field(path="/action:Scalars:scalars", decode=NumericDecoder(), window=(0, 19)),
        "state": Field(path="/state:Scalars:scalars", decode=NumericDecoder(), window=(0, 19)),
        "reward": Field(path="/reward:Scalars:scalars", decode=NumericDecoder(), window=(-5, 0)),
    }
    assert _grouping(fields) == [["action", "state"], ["reward"]]


def test_anchored_field_gets_its_own_group() -> None:
    fields = {
        "video": Field(path="/camera:VideoStream:sample", decode=VideoFrameDecoder()),
        "image": Field(path="/wrist:EncodedImage:blob", decode=ImageDecoder()),
    }
    assert _grouping(fields) == [["image"], ["video"]]


def test_windowed_anchored_decoder_groups_by_window() -> None:
    """An explicit window disables keyframe anchoring, so the window alone shapes the group."""
    fields = {
        "video": Field(path="/camera:VideoStream:sample", decode=VideoFrameDecoder(), window=(0, 9)),
        "action": Field(path="/action:Scalars:scalars", decode=NumericDecoder(), window=(0, 9)),
    }
    assert _grouping(fields) == [["action", "video"]]

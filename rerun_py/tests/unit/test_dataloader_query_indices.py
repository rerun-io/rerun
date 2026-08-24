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
    _content_filter_for_paths,
    _derive_content_filter,
)
from rerun.experimental.dataloader.decoders import ImageDecoder, NumericDecoder, VideoFrameDecoder


def _segment(segment_id: str, index_start: int, num_samples: int, ns_per_sample: int) -> SegmentMetadata:
    return SegmentMetadata(
        segment_id=segment_id,
        index_start=index_start,
        index_end=index_start + (num_samples - 1) * ns_per_sample,
        num_samples=num_samples,
    )


def _targets(
    sample_index: SampleIndex,
    count: int,
    fields: dict[str, Field] | None = None,
    *,
    prior_keyframes: dict[str, int] | None = None,
) -> list[Target]:
    """`Target`s for the first `count` global indices."""
    fields = fields or {"x": Field(path="/x", decode=NumericDecoder())}
    located = (sample_index.global_to_local(i) for i in range(count))
    return [
        _build_target(
            sample_position=position,
            segment=segment,
            index_value=value,
            fields=fields,
            sample_index=sample_index,
            prior_keyframes=prior_keyframes,
        )
        for position, (segment, value) in enumerate(located)
    ]


def _requests(targets: list[Target]) -> dict[str, list[FieldFetchRequest]]:
    return {key: [target.fetch_requests[key] for target in targets] for key in targets[0].fetch_requests}


def test_content_filter_preserves_escaped_colons_in_entity_paths() -> None:
    fields = {
        "video": Field(path=r"/videos/head\:left:VideoStream:sample", decode=VideoFrameDecoder()),
        "state": Field(path=r"/robot/state:Scalars:scalars", decode=NumericDecoder()),
    }

    assert _derive_content_filter(fields) == [r"/robot/state/**", r"/videos/head\:left/**"]
    assert _content_filter_for_paths([
        r"/videos/head\:left:VideoStream:is_keyframe",
    ]) == [r"/videos/head\:left/**"]


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
    prior_keyframes = {key: 0 for key, field in fields.items() if field.prior_keyframe_path is not None}
    plans = _build_query_plans(
        _targets(sample_index, 1, fields, prior_keyframes=prior_keyframes),
        fields,
        sample_index=sample_index,
    )
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
    assert values.tolist() == [10, 12]
    assert plans[0].fetch_requests["action"] == [targets[0].fetch_requests["action"]]


def test_temporal_window_uses_explicit_second_offsets_without_grid_interpolation() -> None:
    second = 1_000_000_000
    segment = _segment("seg-a", index_start=10 * second, num_samples=1, ns_per_sample=second)
    sample_index = SampleIndex([segment], ns_per_sample=second, ns_dtype="datetime64[ns]")
    field = Field(path="/state:Scalars:scalars", decode=NumericDecoder(), window=(-2.5, 0.0))
    target = _build_target(
        sample_position=0,
        segment=segment,
        index_value=np.datetime64(10, "s"),
        fields={"state": field},
        sample_index=sample_index,
    )

    result = _build_query_indices(
        {"state": [target.fetch_requests["state"]]},
        sample_index=sample_index,
    )

    values = result["seg-a"]
    assert isinstance(values, pa.Array)
    assert values.cast(pa.int64()).to_pylist() == [7_500_000_000, 10_000_000_000]


def test_context_query_plan_contains_contiguous_decode_range() -> None:
    sample_index = SampleIndex([SegmentMetadata(segment_id="seg-a", index_start=0, index_end=20, num_samples=21)])
    field = Field(path="/camera:VideoStream:sample", decode=VideoFrameDecoder(), window=(-2, 2))
    target = _build_target(
        sample_position=0,
        segment=sample_index.segments[0],
        index_value=10,
        fields={"video": field},
        sample_index=sample_index,
        prior_keyframes={"video": 3},
    )

    (plan,) = _build_query_plans([target], {"video": field}, sample_index=sample_index)

    assert plan.query_indices == {}
    assert plan.query_ranges == {"seg-a": [(3, 12)]}


def test_video_decode_ranges_remain_scoped_to_their_segments() -> None:
    segments = [
        SegmentMetadata(segment_id="seg-a", index_start=10, index_end=10, num_samples=1),
        SegmentMetadata(segment_id="seg-b", index_start=100, index_end=100, num_samples=1),
    ]
    sample_index = SampleIndex(segments)
    field = Field(path="/camera:VideoStream:sample", decode=VideoFrameDecoder())
    targets = [
        _build_target(
            sample_position=position,
            segment=segment,
            index_value=segment.index_start,
            fields={"video": field},
            sample_index=sample_index,
            prior_keyframes={"video": segment.index_start - 2},
        )
        for position, segment in enumerate(segments)
    ]

    (plan,) = _build_query_plans(targets, {"video": field}, sample_index=sample_index)

    assert plan.query_indices == {}
    assert plan.query_ranges == {
        "seg-a": [(8, 10)],
        "seg-b": [(98, 100)],
    }


def test_windowed_field_does_not_share_group_with_unwindowed() -> None:
    """A shared query would ship the unwindowed image at every index value of the action's window."""
    fields = {
        "image": Field(path="/camera:EncodedImage:blob", decode=ImageDecoder()),
        "action": Field(path="/action:Scalars:scalars", decode=NumericDecoder(), window=tuple(range(20))),
    }
    assert _grouping(fields) == [["action"], ["image"]]


def test_same_window_fields_share_a_group() -> None:
    fields = {
        "action": Field(path="/action:Scalars:scalars", decode=NumericDecoder(), window=tuple(range(20))),
        "state": Field(path="/state:Scalars:scalars", decode=NumericDecoder(), window=tuple(range(20))),
        "reward": Field(path="/reward:Scalars:scalars", decode=NumericDecoder(), window=(-5, 0)),
    }
    assert _grouping(fields) == [["action", "state"], ["reward"]]


def test_contiguous_field_gets_its_own_group() -> None:
    fields = {
        "video": Field(path="/camera:VideoStream:sample", decode=VideoFrameDecoder()),
        "image": Field(path="/wrist:EncodedImage:blob", decode=ImageDecoder()),
    }
    assert _grouping(fields) == [["image"], ["video"]]


def test_windowed_context_decoder_keeps_its_own_group() -> None:
    """A context-aware decoder remains range-fetched with explicit output offsets."""
    fields = {
        "video": Field(path="/camera:VideoStream:sample", decode=VideoFrameDecoder(), window=(0, 9)),
        "action": Field(path="/action:Scalars:scalars", decode=NumericDecoder(), window=(0, 9)),
    }
    assert _grouping(fields) == [["action"], ["video"]]

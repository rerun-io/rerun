"""Tests for the query-shaping helpers `_build_query_indices` and `_build_query_plans`."""

from __future__ import annotations

from types import SimpleNamespace
from typing import cast

import numpy as np
import pyarrow as pa
import pytest
import torch
from rerun.experimental.dataloader import (
    DataSource,
    Field,
    FixedRateSampling,
    Manifest,
    NoShuffle,
    RerunIterableDataset,
    RerunMapDataset,
    TimeAlignedSampling,
    _iterable_dataset as iterable_dataset,
    _map_dataset as map_dataset,
)
from rerun.experimental.dataloader._sample_index import SampleIndex, SegmentMetadata
from rerun.experimental.dataloader._utils import (
    FetchedBlock,
    FetchedGroup,
    FieldFetchRequest,
    QueryPlan,
    Target,
    _build_query_indices,
    _build_query_plans,
    _build_target,
    _build_targets,
    _content_filter_for_paths,
    _decode_iter,
    _derive_content_filter,
    _expand_frame_ranges,
    _index_fetched_block,
    _resolve_decode_requests_in_block,
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


@pytest.mark.parametrize("timeline_sampling", [None, TimeAlignedSampling(keyframe_interval=10)])
def test_aligned_sampling_requires_exact_numeric_indices_and_preserves_window_order(
    timeline_sampling: TimeAlignedSampling | None,
) -> None:
    sample_index = SampleIndex([_segment("a", 2, 3, 1)])
    fields = {"state": Field("/state:Scalars:scalars", decode=NumericDecoder(), window=(0, -2, 0))}
    targets = _targets(sample_index, 3, fields)
    (plan,) = _build_query_plans(targets, fields, sample_index=sample_index, timeline_sampling=timeline_sampling)
    table = pa.table({
        "frame": [0, 2, 3],
        "rerun_segment_id": ["a", "a", "a"],
        "state": pa.array([[0.0], [20.0], [30.0]], type=pa.list_(pa.float64())),
    })
    fetched = FetchedBlock(targets, [FetchedGroup(plan.fields, plan.fetch_requests, table)])

    prepared = _resolve_decode_requests_in_block(_index_fetched_block(fetched, "frame"))
    first, missing_middle, missing_end = list(
        _decode_iter(prepared=prepared, decoders={"state": fields["state"].decode})
    )

    assert plan.fill_latest_at is (timeline_sampling is None)
    assert all(request.fill_latest_at is (timeline_sampling is None) for request in plan.fetch_requests["state"])
    assert all(target.fetch_requests["state"].fill_latest_at for target in targets)
    assert plan.query_indices["a"].tolist() == [0, 1, 2, 3, 4]
    first_state = first["state"]
    assert isinstance(first_state, torch.Tensor)
    assert first_state.tolist() == [[20.0], [0.0], [20.0]]
    if timeline_sampling is not None:
        assert missing_middle["state"] is None
        assert missing_end["state"] is None
    else:
        for sample, expected in [(missing_middle, [[30.0], [0.0], [30.0]]), (missing_end, [[30.0], [20.0], [30.0]])]:
            state = sample["state"]
            assert isinstance(state, torch.Tensor)
            assert state.tolist() == expected


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


def test_aligned_video_queries_fetch_every_frame_without_changing_decoder_requests() -> None:
    segments = [_segment("seg-a", 0, 31, 1), _segment("seg-b", 100, 31, 1)]
    sample_index = SampleIndex(segments)
    field = Field(path="/camera:VideoStream:sample", decode=VideoFrameDecoder(), window=(-2, 0))
    targets = [
        _build_target(
            sample_position=position,
            segment=segments[segment],
            index_value=value,
            fields={"video": field},
            sample_index=sample_index,
            prior_keyframes={"video": keyframe},
        )
        for position, (segment, value, keyframe) in enumerate([
            (0, 10, 6),
            (1, 102, 100),
            (0, 8, 3),
            (0, 15, 13),
            (0, 8, 3),
        ])
    ]

    (original,) = _build_query_plans(targets, {"video": field}, sample_index=sample_index)
    (expanded,) = _build_query_plans(
        targets,
        {"video": field},
        sample_index=sample_index,
        timeline_sampling=TimeAlignedSampling(keyframe_interval=10),
    )

    assert original.query_indices == {}
    assert original.query_ranges == {"seg-a": [(3, 10), (13, 15)], "seg-b": [(100, 102)]}
    assert expanded.query_ranges == {}
    expected = {
        "seg-a": [3, 4, 5, 6, 7, 8, 9, 10, 13, 14, 15],
        "seg-b": [100, 101, 102],
    }
    assert expanded.query_indices.keys() == expected.keys()
    for segment, values in expanded.query_indices.items():
        assert isinstance(values, np.ndarray) and values.dtype == np.int64
        assert values.tolist() == expected[segment]
    assert expanded.fill_latest_at is False
    assert expanded.fields["video"] is field
    for before, after in zip(original.fetch_requests["video"], expanded.fetch_requests["video"], strict=True):
        assert after is before
        assert after.requires_contiguous_fetch and after.starts_at_keyframe
        assert after.output_index_values == (int(after.index_value) - 2, after.index_value)


@pytest.mark.parametrize("interval", [None, 0, -1, 1.5, True])
def test_time_aligned_sampling_requires_a_positive_integer_interval(interval: int | float | None) -> None:
    with pytest.raises(ValueError, match="positive integer"):
        TimeAlignedSampling(keyframe_interval=cast("int", interval))


@pytest.mark.parametrize(
    ("ns_dtype", "ns_per_sample"),
    [("datetime64[ns]", 1_000_000_000), ("timedelta64[ns]", 1_000_000_000), (None, 2)],
)
def test_time_aligned_sampling_rejects_temporal_and_fixed_rate_grids(ns_dtype: str | None, ns_per_sample: int) -> None:
    sample_index = SampleIndex([_segment("seg-a", 0, 3, ns_per_sample)], ns_dtype=ns_dtype, ns_per_sample=ns_per_sample)
    fields = {"video": Field(path="/camera:VideoStream:sample", decode=VideoFrameDecoder())}
    targets = _targets(sample_index, 1, fields, prior_keyframes={"video": 0})

    with pytest.raises(ValueError, match="integer timeline"):
        _build_query_plans(
            targets, fields, sample_index=sample_index, timeline_sampling=TimeAlignedSampling(keyframe_interval=10)
        )


@pytest.mark.parametrize("excess", [0, 1])
def test_aligned_video_query_budget_applies_across_segments_before_expansion(
    excess: int, monkeypatch: pytest.MonkeyPatch
) -> None:
    segments = [_segment("seg-a", 32_767, 1, 1), _segment("seg-b", 32_767 + excess, 1, 1)]
    sample_index = SampleIndex(segments)
    fields = {"video": Field(path="/camera:VideoStream:sample", decode=VideoFrameDecoder())}
    targets = _targets(sample_index, 2, fields, prior_keyframes={"video": 0})
    if excess:

        def reject_expansion(*_args: object, **_kwargs: object) -> None:
            pytest.fail("An oversized query must fall back before enumerating any segment")

        monkeypatch.setattr(np, "arange", reject_expansion)

    (plan,) = _build_query_plans(
        targets, fields, sample_index=sample_index, timeline_sampling=TimeAlignedSampling(keyframe_interval=10)
    )

    if excess:
        assert plan.query_indices == {}
        assert plan.query_ranges == {"seg-a": [(0, 32_767)], "seg-b": [(0, 32_768)]}
    else:
        assert plan.query_ranges == {}
        assert sum(len(values) for values in plan.query_indices.values()) == 65_536
        for values in plan.query_indices.values():
            np.testing.assert_array_equal(values, np.arange(32_768, dtype=np.int64))


def test_dense_frame_expansion_preserves_signed_int64_endpoints() -> None:
    minimum, maximum = -(1 << 63), (1 << 63) - 1
    result = _expand_frame_ranges({"seg-a": [(minimum, minimum + 2)], "seg-b": [(maximum - 2, maximum)]})

    assert result is not None
    assert result["seg-a"].dtype == result["seg-b"].dtype == np.int64
    assert result["seg-a"].tolist() == [minimum, minimum + 1, minimum + 2]
    assert result["seg-b"].tolist() == [maximum - 2, maximum - 1, maximum]


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


def test_aligned_decode_ranges_match_real_keyframes_across_windows_and_segments() -> None:
    sample_index = SampleIndex([_segment("a", 0, 32, 1), _segment("b", 0, 32, 1)])
    fields = {
        "current": Field("/current:VideoStream:sample", decode=VideoFrameDecoder()),
        "history": Field("/history:VideoStream:sample", decode=VideoFrameDecoder(), window=(0, -5, -2)),
        "future": Field("/future:VideoStream:sample", decode=VideoFrameDecoder(), window=(0, 3)),
        "actions": Field("/actions:Scalars:scalars", decode=NumericDecoder(), window=(1, 50)),
    }
    keyframes = {
        key: {segment.segment_id: np.array([0, 10, 20, 30]) for segment in sample_index.segments}
        for key in ("current", "history", "future")
    }
    located = [(segment, value) for segment in sample_index.segments for value in (0, 5, 9, 10, 11, 15, 20, 21)]

    expected = _build_targets(located, keyframes, fields=fields, sample_index=sample_index)
    actual = _build_targets(
        located,
        keyframes,
        fields=fields,
        sample_index=sample_index,
        timeline_sampling=TimeAlignedSampling(keyframe_interval=10),
    )

    assert actual == expected
    assert actual[0].fetch_requests["current"].decode_index_range == (0, 0)
    assert actual[0].fetch_requests["history"].starts_at_keyframe is False
    assert actual[3].fetch_requests["current"].decode_index_range == (10, 10)
    assert actual[3].fetch_requests["history"].decode_index_range == (0, 10)
    assert actual[4].fetch_requests["history"].output_index_values == (11, 6, 9)
    assert actual[4].fetch_requests["actions"].decode_index_range == (12, 61)


def test_aligned_keyframe_calculation_preserves_large_integer_precision() -> None:
    anchor = (1 << 53) + 7
    sample_index = SampleIndex([_segment("a", anchor, 1, 1)])
    fields = {"video": Field("/camera:VideoStream:sample", decode=VideoFrameDecoder())}

    (target,) = _build_targets(
        [(sample_index.segments[0], anchor)],
        {"video": {"a": np.array([9_007_199_254_740_990], dtype=np.int64)}},
        fields=fields,
        sample_index=sample_index,
        timeline_sampling=TimeAlignedSampling(keyframe_interval=10),
    )

    assert target.fetch_requests["video"].decode_index_range == (9_007_199_254_740_990, anchor)


@pytest.mark.parametrize(
    "keyframes",
    [
        pytest.param({}, id="missing-field"),
        pytest.param({"video": {}}, id="missing-segment"),
        pytest.param({"video": {"a": np.array([], dtype=np.int64)}}, id="empty-markers"),
        pytest.param({"video": {"other": np.array([0, 10])}}, id="other-segment-only"),
    ],
)
def test_aligned_sampling_plans_decode_ranges_without_keyframe_discovery(
    keyframes: dict[str, dict[str, np.ndarray]],
) -> None:
    sample_index = SampleIndex([_segment("a", 0, 32, 1)])
    fields = {"video": Field("/camera:VideoStream:sample", decode=VideoFrameDecoder())}

    (target,) = _build_targets(
        [(sample_index.segments[0], 11)],
        keyframes,
        fields=fields,
        sample_index=sample_index,
        timeline_sampling=TimeAlignedSampling(keyframe_interval=10),
    )

    assert target.fetch_requests["video"].decode_index_range == (10, 11)
    assert target.fetch_requests["video"].starts_at_keyframe


@pytest.mark.parametrize("batched", [False, True])
def test_aligned_sampling_plans_each_boundary_independently_of_batching(batched: bool) -> None:
    sample_index = SampleIndex([_segment("a", 0, 32, 1)])
    fields = {"video": Field("/camera:VideoStream:sample", decode=VideoFrameDecoder())}
    located = [(sample_index.segments[0], value) for value in ((9, 11) if batched else (11,))]

    targets = _build_targets(
        located,
        {"video": {"a": np.array([0, 7, 20])}},
        fields=fields,
        sample_index=sample_index,
        timeline_sampling=TimeAlignedSampling(keyframe_interval=10),
    )

    expected = [(0, 9), (10, 11)] if batched else [(10, 11)]
    assert [target.fetch_requests["video"].decode_index_range for target in targets] == expected


def test_aligned_sampling_allows_extra_keyframe_markers_between_boundaries() -> None:
    sample_index = SampleIndex([_segment("a", 0, 32, 1)])
    fields = {"video": Field("/camera:VideoStream:sample", decode=VideoFrameDecoder())}

    targets = _build_targets(
        [(sample_index.segments[0], value) for value in (7, 17)],
        {"video": {"a": np.array([0, 3, 10, 14, 20, 30])}},
        fields=fields,
        sample_index=sample_index,
        timeline_sampling=TimeAlignedSampling(keyframe_interval=10),
    )

    assert [target.fetch_requests["video"].decode_index_range for target in targets] == [(0, 7), (10, 17)]


@pytest.mark.filterwarnings("ignore::RuntimeWarning")
@pytest.mark.parametrize("dataset_type", [RerunIterableDataset, RerunMapDataset])
@pytest.mark.parametrize("timeline_sampling", [None, TimeAlignedSampling(keyframe_interval=10)])
def test_only_regular_live_sampling_discovers_keyframes(
    monkeypatch: pytest.MonkeyPatch,
    dataset_type: type[RerunIterableDataset] | type[RerunMapDataset],
    timeline_sampling: TimeAlignedSampling | None,
) -> None:
    module = iterable_dataset if dataset_type is RerunIterableDataset else map_dataset
    sample_index = SampleIndex([_segment("a", 9, 3, 1)])
    fields = {"video": Field("/camera:VideoStream:sample", decode=VideoFrameDecoder())}
    source = cast(
        "DataSource",
        SimpleNamespace(
            dataset=SimpleNamespace(
                catalog=SimpleNamespace(url="rerun+http://fake", _internal=SimpleNamespace(token=None)), name="ds"
            )
        ),
    )
    discovery_calls = []
    plans_seen: list[QueryPlan] = []

    def discover(**kwargs: object) -> dict[str, dict[str, np.ndarray]]:
        discovery_calls.append(kwargs)
        return {"video": {"a": np.array([0, 10])}}

    def fetch(plans: list[QueryPlan], **_: object) -> list[FetchedGroup]:
        plans_seen.extend(plans)
        return []

    monkeypatch.setattr(SampleIndex, "build", lambda *_args, **_kwargs: sample_index)
    monkeypatch.setattr(module._WorkerConnection, "ensure", lambda _self: (None, {}))
    monkeypatch.setattr(module, "_discover_prior_keyframes", discover)
    monkeypatch.setattr(module, "_fetch_queries_parallel", fetch)
    monkeypatch.setattr(module, "_resolve_decode_requests_in_block", lambda indexed: indexed)
    monkeypatch.setattr(
        module, "_decode_iter", lambda *, prepared, **_: ({"video": target.index_value} for target in prepared.targets)
    )
    if dataset_type is RerunIterableDataset:
        samples = list(
            RerunIterableDataset(
                source, "frame", fields, timeline_sampling=timeline_sampling, shuffle_strategy=NoShuffle()
            )
        )
    else:
        samples = RerunMapDataset(source, "frame", fields, timeline_sampling=timeline_sampling).__getitems__([0, 1, 2])

    assert samples == [{"video": 9}, {"video": 10}, {"video": 11}]
    assert len(discovery_calls) == (1 if timeline_sampling is None else 0)
    (plan,) = plans_seen
    assert [request.decode_index_range for request in plan.fetch_requests["video"]] == [(0, 9), (10, 10), (10, 11)]
    if timeline_sampling is not None:
        assert plan.query_ranges == {}
        assert plan.query_indices["a"].tolist() == list(range(12))
    else:
        assert plan.query_indices == {}
        assert plan.query_ranges == {"a": [(0, 11)]}


@pytest.mark.parametrize("dataset_type", [RerunIterableDataset, RerunMapDataset])
@pytest.mark.parametrize(
    ("timeline_sampling", "error"),
    [
        (None, None),
        (FixedRateSampling(rate_hz=1), None),
        (FixedRateSampling(rate_hz=2), "sampling rate"),
        (TimeAlignedSampling(keyframe_interval=10), "integer timeline"),
    ],
)
def test_manifest_sampling_preserves_the_recorded_time_grid(
    dataset_type: type[RerunIterableDataset] | type[RerunMapDataset],
    timeline_sampling: TimeAlignedSampling | FixedRateSampling | None,
    error: str | None,
) -> None:
    manifest = Manifest._from_arrow(
        pa.table({}).replace_schema_metadata({
            "index_name": "time",
            "ns_dtype": "datetime64[ns]",
            "ns_per_sample": "1000000000",
        })
    )
    source = cast(
        "DataSource",
        SimpleNamespace(
            dataset=SimpleNamespace(
                catalog=SimpleNamespace(url="unused", _internal=SimpleNamespace(token=None)), name="unused"
            )
        ),
    )

    if error is not None:
        with pytest.raises(ValueError, match=error):
            dataset_type.from_manifest(manifest, source, {}, timeline_sampling=timeline_sampling)
    else:
        dataset = dataset_type.from_manifest(manifest, source, {}, timeline_sampling=timeline_sampling)
        assert dataset.sample_index.ns_dtype == "datetime64[ns]"
        assert dataset.sample_index.ns_per_sample == 1_000_000_000


@pytest.mark.parametrize("dataset_type", [RerunIterableDataset, RerunMapDataset])
def test_manifest_decode_anchors_are_preserved_with_an_aligned_layout(
    monkeypatch: pytest.MonkeyPatch,
    dataset_type: type[RerunIterableDataset] | type[RerunMapDataset],
) -> None:
    fields = {
        "video": Field("/camera:VideoStream:sample", decode=VideoFrameDecoder(), window=(-2, 0)),
        "state": Field("/state:Scalars:scalars", decode=NumericDecoder()),
    }
    rows = pa.table({
        "rank": [0],
        "worker": [0],
        "fetch_group": [0],
        "emit_rank": [0],
        "segment_id": ["a"],
        "anchor": [8],
        "video": [{"lo": 3, "hi": 8}],
        "state": [{"lo": 8, "hi": 8}],
    })
    manifest = Manifest._from_arrow(rows.replace_schema_metadata({"index_name": "frame"}))
    source = cast(
        "DataSource",
        SimpleNamespace(
            dataset=SimpleNamespace(
                catalog=SimpleNamespace(url="unused", _internal=SimpleNamespace(token=None)), name="unused"
            )
        ),
    )
    module = iterable_dataset if dataset_type is RerunIterableDataset else map_dataset
    monkeypatch.setattr(module._WorkerConnection, "ensure", lambda _: (None, {}))
    captured: list[QueryPlan] = []

    def capture_queries(plans: list[QueryPlan], **_: object) -> list[FetchedGroup]:
        captured.extend(plans)
        raise RuntimeError("query captured before catalog I/O")

    monkeypatch.setattr(module, "_fetch_queries_parallel", capture_queries)
    timeline_sampling = TimeAlignedSampling(keyframe_interval=10)
    with pytest.raises(RuntimeError, match="query captured before catalog I/O"):
        if dataset_type is RerunIterableDataset:
            list(RerunIterableDataset.from_manifest(manifest, source, fields, timeline_sampling=timeline_sampling))
        else:
            RerunMapDataset.from_manifest(manifest, source, fields, timeline_sampling=timeline_sampling)[0]

    (plan,) = [plan for plan in captured if "video" in plan.fields]
    (state_plan,) = [plan for plan in captured if "state" in plan.fields]
    (request,) = plan.fetch_requests["video"]
    assert request.decode_index_range == (3, 8)
    assert request.output_index_values == (6, 8)
    assert request.starts_at_keyframe
    assert plan.query_ranges == {}
    assert plan.query_indices["a"].tolist() == [3, 4, 5, 6, 7, 8]
    assert state_plan.fill_latest_at is False
    assert state_plan.fetch_requests["state"][0].fill_latest_at is False
    assert state_plan.query_indices["a"].tolist() == [8]

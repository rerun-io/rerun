"""Compare bounded live keyframe discovery with complete marker history."""

from __future__ import annotations

from typing import TYPE_CHECKING, Any, cast

import numpy as np
import pyarrow as pa
import pytest
from datafusion import SessionContext
from rerun.experimental.dataloader import Field, NumericDecoder, VideoFrameDecoder, _utils as utils
from rerun.experimental.dataloader._sample_index import SampleIndex, SegmentMetadata, _index_value_to_int
from rerun.experimental.dataloader.manifest import _manifest_build as manifest_build

if TYPE_CHECKING:
    from collections.abc import Mapping, Sequence

    from datafusion import DataFrame
    from rerun.catalog import DatasetEntry
    from rerun.experimental.dataloader._sample_index import IndexValue

_LEFT = "/left:VideoStream:is_keyframe"
_RIGHT = "/right:VideoStream:is_keyframe"


class _MarkerDataset:
    def __init__(
        self,
        markers: Mapping[str, Mapping[tuple[str, int], Sequence[bool | None] | None]],
        *,
        index_type: pa.DataType | None = None,
    ) -> None:
        rows = sorted({row for values in markers.values() for row in values})
        self.table = pa.table({
            "index": pa.array([value for _, value in rows], type=index_type or pa.int64()),
            "rerun_segment_id": pa.array([segment for segment, _ in rows], type=pa.string()),
            **{
                path: pa.array([values.get(row) for row in rows], type=pa.list_(pa.bool_()))
                for path, values in markers.items()
            },
        })
        self.calls: list[dict[str, Any]] = []
        self.contents: list[list[str]] = []
        self.segments: list[str] | None = None
        self.ctx = SessionContext()

    def filter_contents(self, paths: list[str]) -> _MarkerDataset:
        self.contents.append(paths)
        return self

    def filter_segments(self, segments: list[str]) -> _MarkerDataset:
        self.segments = segments
        return self

    def reader(self, **kwargs: Any) -> DataFrame:
        self.calls.append({**kwargs, "segments": list(self.segments or [])})
        exact = kwargs.get("using_index_values")
        allowed = None if exact is None else {segment: set(values.tolist()) for segment, values in exact.items()}
        ids = self.table.column("rerun_segment_id").to_pylist()
        values = self.table.column("index").cast(pa.int64()).to_pylist()
        mask = [
            (self.segments is None or segment in self.segments)
            and (allowed is None or value in allowed.get(segment, set()))
            for segment, value in zip(ids, values, strict=True)
        ]
        table = self.table.filter(pa.array(mask, type=pa.bool_()))
        batches = table.to_batches() or [
            pa.RecordBatch.from_arrays([column.combine_chunks() for column in table.columns], schema=table.schema)
        ]
        return self.ctx.create_dataframe([batches])


def _discover(
    dataset: _MarkerDataset,
    fields: dict[str, Field],
    sample_index: SampleIndex,
    located: Sequence[tuple[SegmentMetadata, IndexValue]],
) -> dict[str, dict[str, np.ndarray]]:
    return utils._discover_prior_keyframes(
        view=cast("DatasetEntry", dataset), index="index", fields=fields, located=located, sample_index=sample_index
    )


def _assert_matches_full_scan(
    dataset: _MarkerDataset,
    fields: dict[str, Field],
    sample_index: SampleIndex,
    located: Sequence[tuple[SegmentMetadata, IndexValue]],
) -> dict[str, dict[str, np.ndarray]]:
    expected = utils._fetch_prior_keyframes(
        view=cast("DatasetEntry", dataset), index="index", fields=fields, located=located, sample_index=sample_index
    )
    dataset.calls.clear()
    actual = _discover(dataset, fields, sample_index, located)
    for segment, anchor in located:
        for key, field in fields.items():
            if field.prior_keyframe_path is None:
                continue
            target = min(_index_value_to_int(value) for value in sample_index.output_index_values(anchor, field))
            assert utils._prior_keyframe(actual[key].get(segment.segment_id), target) == utils._prior_keyframe(
                expected[key].get(segment.segment_id), target
            )
    return actual


def test_irregular_camera_gops_aliases_and_windows_match_full_scan() -> None:
    segments = [SegmentMetadata(name, -10, 220, 231, index_min=-30) for name in ("a", "b")]
    fields = {
        "left": Field("/left:VideoStream:sample", decode=VideoFrameDecoder()),
        "history": Field("/left:VideoStream:sample", decode=VideoFrameDecoder(), window=(0, -20, -3)),
        "right": Field("/right:VideoStream:sample", decode=VideoFrameDecoder(), window=(-2, 0)),
    }
    dataset = _MarkerDataset({
        _LEFT: {(name, value): [True] for name in ("a", "b") for value in (-30, 0, 7, 40, 93, 180)},
        _RIGHT: {(name, value): [True] for name in ("a", "b") for value in (-30, 11, 67, 142, 199)},
    })
    located = [(segments[0], 200), (segments[1], 170), (segments[0], 60), (segments[0], 200)]

    result = _assert_matches_full_scan(dataset, fields, SampleIndex(segments), located)

    assert result["left"] is result["history"]
    assert all(call["fill_latest_at"] is False for call in dataset.calls)


def test_discovery_searches_gaps_before_accepting_another_targets_old_marker() -> None:
    segment = SegmentMetadata("a", 0, 100, 101, index_min=0)
    fields = {"video": Field("/left:VideoStream:sample", decode=VideoFrameDecoder())}
    dataset = _MarkerDataset({_LEFT: {("a", 10): [True], ("a", 70): [True]}})

    result = _assert_matches_full_scan(dataset, fields, SampleIndex([segment]), [(segment, 20), (segment, 100)])

    assert utils._prior_keyframe(result["video"]["a"], 100) == 70
    queried = [set(call["using_index_values"]["a"].tolist()) for call in dataset.calls]
    assert any(70 in values for values in queried)
    assert all(not (values & later) for position, values in enumerate(queried) for later in queried[position + 1 :])


def test_overlapping_windows_are_queried_once_and_resolved_segments_stop_expanding() -> None:
    fast, slow = [SegmentMetadata(name, 0, 105, 106, index_min=0) for name in ("fast", "slow")]
    fields = {"video": Field("/left:VideoStream:sample", decode=VideoFrameDecoder())}
    dataset = _MarkerDataset({_LEFT: {("fast", 90): [True], ("slow", 0): [True]}})

    _assert_matches_full_scan(dataset, fields, SampleIndex([fast, slow]), [(fast, 100), (fast, 105), (slow, 100)])

    assert len(dataset.calls) > 1
    assert dataset.calls[0]["using_index_values"]["fast"].tolist() == list(range(85, 106))
    assert all("fast" not in call["segments"] for call in dataset.calls[1:])
    previous: set[int] = set()
    for call in dataset.calls:
        values = set(call["using_index_values"].get("slow", np.array([], dtype=np.int64)).tolist())
        assert not values & previous
        previous.update(values)
    assert previous == set(range(101))


@pytest.mark.parametrize("explicit_floor", [False, True])
def test_discovery_reaches_raw_floor_before_global_window_trim(explicit_floor: bool) -> None:
    segment = SegmentMetadata("a", 50, 100, 51, index_min=-50 if explicit_floor else None)
    fields = {
        "video": Field("/left:VideoStream:sample", decode=VideoFrameDecoder()),
        "history": Field("/state:Scalars:scalars", decode=NumericDecoder(), window=(-100, 0)),
    }
    dataset = _MarkerDataset({_LEFT: {("a", -20): [True]}})

    result = _discover(dataset, fields, SampleIndex([segment]), [(segment, 50)])

    assert result["video"]["a"].tolist() == [-20]
    assert min(value for call in dataset.calls for value in call["using_index_values"]["a"]) == -50


@pytest.mark.parametrize(
    "ns_dtype,index_type", [("datetime64[ns]", pa.timestamp("ns")), ("timedelta64[ns]", pa.duration("ns"))]
)
def test_temporal_discovery_retains_off_grid_markers(ns_dtype: str, index_type: pa.DataType) -> None:
    segment = SegmentMetadata("a", 5000, 10000, 51, index_min=0)
    sample_index = SampleIndex([segment], ns_per_sample=100, ns_dtype=ns_dtype)
    fields = {"video": Field("/left:VideoStream:sample", decode=VideoFrameDecoder(), window=(-0.0000002, 0))}
    dataset = _MarkerDataset(
        {_LEFT: {("a", 7): [True], ("a", 3499): [True], ("a", 4793): [True]}}, index_type=index_type
    )
    anchor = np.datetime64(5000, "ns") if ns_dtype == "datetime64[ns]" else np.timedelta64(5000, "ns")

    result = _assert_matches_full_scan(dataset, fields, sample_index, [(segment, anchor)])

    assert utils._prior_keyframe(result["video"]["a"], 4800) == 4793
    assert all("using_index_values" not in call for call in dataset.calls)


@pytest.mark.parametrize(
    "ns_dtype,index_type", [("datetime64[ns]", pa.timestamp("ns")), ("timedelta64[ns]", pa.duration("ns"))]
)
@pytest.mark.parametrize("explicit_floor", [False, True])
def test_temporal_discovery_reaches_raw_floor_before_window_trim(
    ns_dtype: str, index_type: pa.DataType, explicit_floor: bool
) -> None:
    segment = SegmentMetadata("a", 5000, 10000, 51, index_min=0 if explicit_floor else None)
    sample_index = SampleIndex([segment], ns_per_sample=100, ns_dtype=ns_dtype)
    fields = {
        "video": Field("/left:VideoStream:sample", decode=VideoFrameDecoder()),
        "history": Field("/state:Scalars:scalars", decode=NumericDecoder(), window=(-0.000005, 0)),
    }
    dataset = _MarkerDataset({_LEFT: {("a", 7): [True]}}, index_type=index_type)
    anchor = np.datetime64(5000, "ns") if ns_dtype == "datetime64[ns]" else np.timedelta64(5000, "ns")

    result = _discover(dataset, fields, sample_index, [(segment, anchor)])

    assert result["video"]["a"].tolist() == [7]
    assert len(dataset.calls) == 3
    assert all("using_index_values" not in call for call in dataset.calls)


@pytest.mark.parametrize("floor,target", [(-100, -10), (-(1 << 63), -(1 << 63) + 30), ((1 << 63) - 40, (1 << 63) - 1)])
def test_integer_discovery_preserves_negative_and_int64_boundary_coordinates(floor: int, target: int) -> None:
    segment = SegmentMetadata("a", floor, target, target - floor + 1, index_min=floor)
    fields = {"video": Field("/left:VideoStream:sample", decode=VideoFrameDecoder())}
    dataset = _MarkerDataset({_LEFT: {("a", floor): [True]}})

    result = _assert_matches_full_scan(dataset, fields, SampleIndex([segment]), [(segment, target)])

    assert utils._prior_keyframe(result["video"]["a"], target) == floor
    for call in dataset.calls:
        values = call["using_index_values"]["a"]
        assert values.dtype == np.int64
        assert int(values.min()) >= floor and int(values.max()) <= target


def test_dense_false_null_empty_markers_terminate_at_floor_without_a_keyframe() -> None:
    segment = SegmentMetadata("a", -20, 100, 121, index_min=-20)
    fields = {"video": Field("/left:VideoStream:sample", decode=VideoFrameDecoder())}
    flags = [[False], None, [], [None]]
    dataset = _MarkerDataset({_LEFT: {("a", index): flags[index % len(flags)] for index in range(-20, 101)}})

    result = _discover(dataset, fields, SampleIndex([segment]), [(segment, 100)])

    assert result == {"video": {}}
    all_values = [int(value) for call in dataset.calls for value in call["using_index_values"]["a"]]
    assert sorted(all_values) == list(range(-20, 101))
    assert len(dataset.calls) == 4


def test_sparse_true_markers_survive_dense_false_null_and_empty_rows() -> None:
    segment = SegmentMetadata("a", 0, 100, 101, index_min=0)
    fields = {"video": Field("/left:VideoStream:sample", decode=VideoFrameDecoder())}
    flags = [[False], None, [], [None]]
    markers = {("a", index): flags[index % len(flags)] for index in range(101)}
    markers.update({("a", 13): [True], ("a", 77): [True]})
    dataset = _MarkerDataset({_LEFT: markers})

    result = _discover(dataset, fields, SampleIndex([segment]), [(segment, 100)])

    assert result["video"]["a"].tolist() == [77]
    assert len(dataset.calls) == 2


def test_large_integer_discovery_round_uses_bounded_range_query() -> None:
    segment = SegmentMetadata("a", 0, 200000, 200001, index_min=0)
    fields = {"video": Field("/left:VideoStream:sample", decode=VideoFrameDecoder())}
    dataset = _MarkerDataset({_LEFT: {("a", 0): [True]}})

    result = _discover(dataset, fields, SampleIndex([segment]), [(segment, 200000)])

    assert result["video"]["a"].tolist() == [0]
    assert all(len(call["using_index_values"]["a"]) <= 65536 for call in dataset.calls if "using_index_values" in call)
    assert any("using_index_values" not in call for call in dataset.calls)


def test_fully_resolved_camera_is_not_projected_in_later_search_rounds() -> None:
    segment = SegmentMetadata("a", 0, 100, 101, index_min=0)
    fields = {
        "left": Field("/left:VideoStream:sample", decode=VideoFrameDecoder()),
        "right": Field("/right:VideoStream:sample", decode=VideoFrameDecoder()),
    }
    dataset = _MarkerDataset({_LEFT: {("a", 99): [True]}, _RIGHT: {("a", 0): [True]}})

    result = _discover(dataset, fields, SampleIndex([segment]), [(segment, 100)])

    assert result["left"]["a"].tolist() == [99]
    assert result["right"]["a"].tolist() == [0]
    assert set(dataset.contents[0]) == {"/left/**", "/right/**"}
    assert all(paths == ["/right/**"] for paths in dataset.contents[1:])


def test_manifest_scan_keeps_complete_history_without_adaptive_discovery(monkeypatch: pytest.MonkeyPatch) -> None:
    segment = SegmentMetadata("a", 0, 100, 101, index_min=0)
    sample_index = SampleIndex([segment])
    fields = {"video": Field("/left:VideoStream:sample", decode=VideoFrameDecoder())}
    dataset = _MarkerDataset({_LEFT: {("a", index): [True] for index in (0, 30, 90)}})

    def reject_adaptive(**_kwargs: object) -> None:
        pytest.fail("Manifest scanning must retain the complete keyframe history")

    monkeypatch.setattr(utils, "_discover_prior_keyframes", reject_adaptive)
    scan = manifest_build._scan(
        view=cast("DatasetEntry", dataset),
        index="index",
        fields=fields,
        sample_index=sample_index,
        segment_maxes=[(segment, 100)],
        required={"video"},
        max_workers=1,
    )

    assert scan.keyframes["video"]["a"].tolist() == [0, 30, 90]
    assert len(dataset.calls) == 1 and "using_index_values" not in dataset.calls[0]

"""Validate aligned keyframe boundaries against the data fetched for each sample."""

from __future__ import annotations

from typing import TYPE_CHECKING, cast

import pyarrow as pa
import pytest
import torch
from rerun.experimental.dataloader import Field, NumericDecoder, TimeAlignedSampling, VideoFrameDecoder, _utils as utils
from rerun.experimental.dataloader._iterable_dataset import _raise_if_incomplete, _skip_incomplete
from rerun.experimental.dataloader._sample_index import SampleIndex, SegmentMetadata
from rerun.experimental.dataloader._utils import (
    FetchedBlock,
    FetchedGroup,
    _build_query_plans,
    _build_targets,
    _decode_iter,
    _index_fetched_block,
    _resolve_decode_requests_in_block,
)

if TYPE_CHECKING:
    from collections.abc import Mapping, Sequence

    from rerun.catalog import DatasetEntry
    from rerun.experimental.dataloader._utils import PreparedBlock, QueryPlan, Target
    from rerun.experimental.dataloader.decoders._base import DecodedSample


def _plans(
    fields: dict[str, Field], located: list[tuple[str, int]], *, index: str = "frame"
) -> tuple[list[Target], list[QueryPlan]]:
    segments = {name: SegmentMetadata(name, 0, 100, 101) for name, _ in located}
    sample_index = SampleIndex(list(segments.values()))
    targets = _build_targets(
        [(segments[name], value) for name, value in located],
        {},
        fields=fields,
        sample_index=sample_index,
        timeline_sampling=TimeAlignedSampling(keyframe_interval=10),
    )
    plans = _build_query_plans(
        targets,
        fields,
        sample_index=sample_index,
        timeline_sampling=TimeAlignedSampling(keyframe_interval=10),
        index=index,
    )
    return targets, plans


def _prepare(
    fields: dict[str, Field],
    located: list[tuple[str, int]],
    rows: list[tuple[str, int]],
    markers: Mapping[str, Mapping[tuple[str, int], Sequence[bool | None] | None]],
    data: dict[str, dict[tuple[str, int], list[float] | None]] | None = None,
) -> tuple[list[Target], PreparedBlock]:
    targets, plans = _plans(fields, located)
    groups = []
    for plan in plans:
        columns = {
            "frame": pa.array([index for _, index in rows], type=pa.int64()),
            "rerun_segment_id": pa.array([segment for segment, _ in rows], type=pa.string()),
        }
        for key in plan.fields:
            overrides = (data or {}).get(key, {})
            columns[key] = pa.array([overrides.get(row, [float(row[1])]) for row in rows], type=pa.list_(pa.float64()))
        for key, alias in plan.keyframe_columns.items():
            columns[alias] = pa.array(
                [markers.get(key, {}).get(row, [False]) for row in rows], type=pa.list_(pa.bool_())
            )
        groups.append(FetchedGroup(plan.fields, plan.fetch_requests, pa.table(columns), plan.keyframe_columns))
    indexed = _index_fetched_block(FetchedBlock(targets, groups), "frame")
    return targets, _resolve_decode_requests_in_block(indexed)


def _decode(prepared: PreparedBlock) -> list[DecodedSample]:
    return list(_decode_iter(prepared=prepared, decoders={key: NumericDecoder() for key in prepared.fields}))


def test_loaded_markers_preserve_windows_duplicates_segments_and_sample_order() -> None:
    fields = {
        "current": Field("/camera:VideoStream:sample", decode=VideoFrameDecoder()),
        "history": Field("/camera:VideoStream:sample", decode=VideoFrameDecoder(), window=(0, -5, -2)),
    }
    located = [("b", 11), ("a", 9), ("a", 11), ("b", 11)]
    rows = [(segment, value) for segment in ("b", "a") for value in reversed(range(12))]
    markers = {key: {(segment, index): [True] for segment in ("a", "b") for index in (0, 10)} for key in fields}

    _, prepared = _prepare(fields, located, rows, markers)
    samples = _decode(prepared)

    for (_, index), sample in zip(located, samples, strict=True):
        current, history = sample["current"], sample["history"]
        assert isinstance(current, torch.Tensor) and isinstance(history, torch.Tensor)
        assert current.tolist() == [float(index)]
        assert history.tolist() == [[float(index)], [float(index - 5)], [float(index - 2)]]


@pytest.mark.parametrize("marker", [None, [], [None], [False]])
def test_bad_boundary_marker_only_invalidates_its_own_target(marker: list[bool | None] | None) -> None:
    fields = {"video": Field("/camera:VideoStream:sample", decode=VideoFrameDecoder())}
    _, prepared = _prepare(
        fields,
        [("a", 11), ("a", 9), ("a", 11)],
        [("a", index) for index in range(12)],
        {"video": {("a", 0): [True], ("a", 7): [True], ("a", 10): marker, ("a", 11): [True]}},
    )

    samples = _decode(prepared)

    assert samples[0]["video"] is samples[2]["video"] is None
    valid = samples[1]["video"]
    assert isinstance(valid, torch.Tensor) and valid.tolist() == [9.0]
    assert [request.sample_position for request in prepared.fields["video"].requests] == [1]


def test_later_true_marker_cannot_replace_a_missing_expected_start_row() -> None:
    fields = {"video": Field("/camera:VideoStream:sample", decode=VideoFrameDecoder())}
    _, prepared = _prepare(
        fields,
        [("a", 11)],
        [("a", 11)],
        {"video": {("a", 11): [True]}},
    )

    assert _decode(prepared) == [{"video": None}]
    assert prepared.fields["video"].requests == []


@pytest.mark.parametrize("value", [None, []])
def test_true_marker_with_missing_start_data_cannot_shift_to_a_later_frame(value: list[float] | None) -> None:
    fields = {"video": Field("/camera:VideoStream:sample", decode=VideoFrameDecoder())}
    _, prepared = _prepare(
        fields,
        [("a", 11)],
        [("a", 10), ("a", 11)],
        {"video": {("a", 10): [True], ("a", 11): [True]}},
        data={"video": {("a", 10): value}},
    )

    assert _decode(prepared) == [{"video": None}]
    assert prepared.fields["video"].requests == []


@pytest.mark.filterwarnings("ignore::RuntimeWarning")
def test_each_camera_and_segment_requires_its_own_marker_before_skip_policy() -> None:
    fields = {key: Field(f"/{key}:VideoStream:sample", decode=VideoFrameDecoder()) for key in ("left", "right")}
    targets, prepared = _prepare(
        fields,
        [("a", 11), ("b", 11), ("a", 20)],
        [(segment, index) for segment in ("a", "b") for index in range(21)],
        {
            "left": {("a", 10): [True], ("b", 10): [True], ("a", 20): [True]},
            "right": {("a", 10): [False], ("b", 10): None, ("a", 20): [True]},
        },
    )
    samples = _decode(prepared)

    assert all(isinstance(sample["left"], torch.Tensor) for sample in samples)
    assert samples[0]["right"] is samples[1]["right"] is None
    kept = list(_skip_incomplete((sample for sample in samples), max_consecutive_skipped_samples=2))
    assert len(kept) == 1 and kept[0] is samples[2]
    with pytest.raises(RuntimeError, match="max_consecutive_skipped_samples=1"):
        list(_skip_incomplete((sample for sample in samples), max_consecutive_skipped_samples=1))
    with pytest.raises(RuntimeError, match="Required fields decoded to nothing: right"):
        _raise_if_incomplete(samples[0], targets[0], {"left", "right"})
    _raise_if_incomplete(samples[0], targets[0], {"left"})


def test_hidden_marker_aliases_cannot_shadow_user_fields_or_the_index() -> None:
    video = Field("/camera:VideoStream:sample", decode=VideoFrameDecoder())
    _, (initial,) = _plans({"video": video}, [("a", 11)])
    occupied = initial.keyframe_columns["video"]
    _, (plan,) = _plans({"video": video, occupied: video}, [("a", 11)])

    assert len(set(plan.keyframe_columns.values())) == 2
    assert not (set(plan.keyframe_columns.values()) & {"video", occupied, "frame", "rerun_segment_id"})
    _, (index_collision,) = _plans({"video": video}, [("a", 11)], index=occupied)
    assert occupied not in index_collision.keyframe_columns.values()


def test_inline_marker_projection_includes_a_custom_decoder_marker_entity(monkeypatch: pytest.MonkeyPatch) -> None:
    class CustomDecoder(NumericDecoder):
        def prior_keyframe_path(self, field_path: str) -> str:
            del field_path
            return "/markers:Flags:is_keyframe"

    class Expression:
        def __init__(self, path: str, alias: str | None = None) -> None:
            self.path, self.name = path, alias

        def alias(self, name: str) -> Expression:
            return Expression(self.path, name)

    contents: list[str] = []
    selections: list[Expression] = []
    reads = []

    class Reader:
        def filter_contents(self, paths: list[str]) -> Reader:
            contents.extend(paths)
            return self

        def filter_segments(self, _segments: list[str]) -> Reader:
            return self

        def reader(self, **kwargs: object) -> Reader:
            reads.append(kwargs)
            return self

        def select(self, *expressions: Expression) -> Reader:
            selections.extend(expressions)
            return self

        def to_arrow_table(self) -> pa.Table:
            return pa.table({})

    fields = {"custom": Field("/values:Samples:value", decode=CustomDecoder())}
    _, (plan,) = _plans(fields, [("a", 11)])
    monkeypatch.setattr(utils, "col", Expression)

    utils._fetch_query(view=cast("DatasetEntry", Reader()), index="frame", plan=plan)

    assert set(contents) == {"/values/**", "/markers/**"}
    assert len(reads) == 1
    assert [(expression.path, expression.name) for expression in selections] == [
        ("frame", None),
        ("rerun_segment_id", None),
        (fields["custom"].path, "custom"),
        ("/markers:Flags:is_keyframe", plan.keyframe_columns["custom"]),
    ]

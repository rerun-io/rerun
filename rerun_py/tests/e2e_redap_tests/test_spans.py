from __future__ import annotations

from datetime import UTC, datetime, timedelta
from typing import TYPE_CHECKING
from uuid import UUID

import numpy as np
import pyarrow as pa
import pytest
import rerun as rr
from datafusion import col
from rerun.catalog import AlreadyExistsError
from rerun.experimental.spans import SpanTable

if TYPE_CHECKING:
    from pathlib import Path
    from typing import Any

    from rerun.catalog import DatasetEntry

    from .conftest import EntryFactory


DATA_SCHEMA = pa.schema([("state", pa.string()), ("score", pa.float32()), ("embedding", pa.list_(pa.float32(), 2))])


@pytest.fixture
def segments(readonly_test_dataset: DatasetEntry) -> tuple[str, str]:
    first, second = sorted(readonly_test_dataset.segment_ids())[:2]
    return first, second


@pytest.fixture
def span_table(
    readonly_test_dataset: DatasetEntry, segments: tuple[str, str], request: pytest.FixtureRequest
) -> SpanTable:
    spans = SpanTable.create(readonly_test_dataset, "time_2", schema=DATA_SCHEMA)
    request.addfinalizer(spans.delete)
    first, second = segments
    spans.write(
        pa.RecordBatch.from_pylist(
            [
                {
                    "rerun_segment_id": first,
                    "rerun_span_id": "a1",
                    "time_2:start": 2,
                    "time_2:end": 10,
                    "state": "grasp",
                    "score": 0.9,
                    "embedding": [1.0, 0.0],
                },
                {"rerun_segment_id": first, "rerun_span_id": "a2", "time_2:start": 2, "time_2:end": 8},
                {"rerun_segment_id": second, "rerun_span_id": "b1", "time_2:start": 12, "time_2:end": 15},
            ],
            schema=spans.arrow_schema(),
        )
    )
    return spans


def test_reader_segment_filters(span_table: SpanTable, segments: tuple[str, str]) -> None:
    selected = span_table.reader().filter(col("rerun_segment_id") == segments[0]).to_arrow_table()

    assert sorted(selected["rerun_span_id"].to_pylist()) == ["a1", "a2"]
    assert span_table.reader().filter(col("rerun_segment_id") == "absent").count() == 0


@pytest.mark.parametrize(
    "index,boundaries",
    [
        (
            "time_2",
            [
                (timedelta(seconds=1.25), 1_250_000_000),
                (np.timedelta64(1_234_567_891_234_567_891, "ns"), 1_234_567_891_234_567_891),
            ],
        ),
        (
            "time_1",
            [
                (datetime(2026, 11, 12, 3, 4, 5, 123_457, tzinfo=UTC), 1_794_452_645_123_457_000),
                (np.datetime64(2**63 - 1, "ns"), 2**63 - 1),
            ],
        ),
    ],
)
def test_scalar_boundaries_round_trip_exactly(
    readonly_test_dataset: DatasetEntry,
    segments: tuple[str, str],
    request: pytest.FixtureRequest,
    index: str,
    boundaries: list[tuple[Any, int]],
) -> None:
    spans = SpanTable.create(readonly_test_dataset, index=index)
    request.addfinalizer(spans.delete)
    expected = {
        spans.append(segment_id=segments[0], start=boundary, end=boundary): nanos for boundary, nanos in boundaries
    }

    rows = spans.table.reader().to_arrow_table()

    for name in (f"{index}:start", f"{index}:end"):
        actual = dict(zip(rows["rerun_span_id"].to_pylist(), rows[name].cast(pa.int64()).to_pylist(), strict=True))
        assert actual == expected, name


def test_append_generates_uuid4_ids(span_table: SpanTable, segments: tuple[str, str]) -> None:
    span_id = span_table.append(segment_id=segments[0], start=timedelta(seconds=3), end=timedelta(seconds=7))

    assert UUID(span_id).version == 4
    assert span_table.table.reader().count() == 4


def test_upsert_replaces_whole_row(span_table: SpanTable, segments: tuple[str, str]) -> None:
    span_table.upsert(
        segment_id=segments[0],
        span_id="a1",
        start=timedelta(seconds=3),
        end=timedelta(seconds=7),
        data={"state": "updated"},
    )

    row = span_table.table.reader().filter(col("rerun_span_id") == "a1").to_arrow_table().to_pylist()
    assert len(row) == 1
    assert row[0]["state"] == "updated" and row[0]["score"] is None and row[0]["embedding"] is None
    assert span_table.table.reader().count() == 3


def test_upsert_inserts_unknown_span_id(span_table: SpanTable, segments: tuple[str, str]) -> None:
    span_id = span_table.upsert(
        segment_id=segments[0], span_id="caller_span", start=timedelta(seconds=3), end=timedelta(seconds=7)
    )

    assert span_id == "caller_span"
    assert sorted(span_table.table.reader().to_arrow_table()["rerun_span_id"].to_pylist()) == [
        "a1",
        "a2",
        "b1",
        "caller_span",
    ]


def test_overwrite_replaces_all_spans_with_generated_ids(span_table: SpanTable, segments: tuple[str, str]) -> None:
    replacements = pa.Table.from_pylist(
        [{"rerun_segment_id": segments[0], "time_2:start": start, "time_2:end": start + 4} for start in (3, 10)],
        schema=span_table.arrow_schema(),
    )

    span_table.write(replacements, mode="overwrite")

    rows = span_table.table.reader().to_arrow_table()
    assert sorted(rows["time_2:start"].cast(pa.int64()).to_pylist()) == [3, 10]
    assert all(UUID(value).version == 4 for value in rows["rerun_span_id"].to_pylist())


def test_overwrite_with_empty_input_clears_table(span_table: SpanTable) -> None:
    span_table.write(pa.Table.from_pylist([], schema=span_table.arrow_schema()), mode="overwrite")

    assert span_table.table.reader().count() == 0


def test_containment_semi_join_returns_each_span_once(span_table: SpanTable, segments: tuple[str, str]) -> None:
    first, second = segments
    span_table.write(
        pa.table({
            "rerun_segment_id": [first, first, first, second],
            "rerun_span_id": ["task_wide", "task_narrow", "grasp_outside", "grasp_other_segment"],
            "time_2:start": pa.array([0, 1, 5, 3], pa.duration("ns")),
            "time_2:end": pa.array([20, 11, 21, 4], pa.duration("ns")),
            "state": ["pick_and_place", "pick_and_place", "grasp", "grasp"],
        })
    )
    spans = span_table.reader()
    grasps = spans.filter(col("state") == "grasp").alias("grasp")
    tasks = spans.filter(col("state") == "pick_and_place").alias("task")

    nested = grasps.join_on(
        tasks,
        grasps.col("rerun_segment_id") == tasks.col("rerun_segment_id"),
        grasps.col("time_2:start") >= tasks.col("time_2:start"),
        grasps.col("time_2:end") <= tasks.col("time_2:end"),
        how="semi",
    ).to_arrow_table()

    assert nested["rerun_span_id"].to_pylist() == ["a1"]


def test_writes_reject_unknown_segments(span_table: SpanTable) -> None:
    with pytest.raises(ValueError, match=r"\['episode_typo'\]"):
        span_table.append(segment_id="episode_typo", start=timedelta(seconds=1), end=timedelta(seconds=2))

    assert span_table.table.reader().count() == 3


def test_open_without_span_table_raises(readonly_test_dataset: DatasetEntry) -> None:
    with pytest.raises(LookupError):
        SpanTable.open(readonly_test_dataset)


def test_create_and_open_by_name(readonly_test_dataset: DatasetEntry, request: pytest.FixtureRequest) -> None:
    default = SpanTable.create(readonly_test_dataset, index="time_2")
    request.addfinalizer(default.delete)
    named = SpanTable.create(readonly_test_dataset, index="time_2", name="custom_spans")
    request.addfinalizer(named.delete)

    assert default.table.name == f"spans.dataset_{readonly_test_dataset.id}".lower()
    assert named.table.name == "custom_spans"
    assert SpanTable.open(readonly_test_dataset, name="custom_spans").table.id == named.table.id
    assert SpanTable.open(readonly_test_dataset).table.id == default.table.id
    with pytest.raises(AlreadyExistsError):
        SpanTable.create(readonly_test_dataset, index="time_2")
    with pytest.raises(AlreadyExistsError):
        SpanTable.create(readonly_test_dataset, index="time_2", name="custom_spans")


# The tests below register on-the-fly recordings, or rename and delete the dataset.


@pytest.fixture
def spans_dataset(entry_factory: EntryFactory, tmp_path: Path) -> DatasetEntry:
    dataset = entry_factory.create_dataset("span_annotations")
    path = tmp_path / "episode_a.rrd"
    with rr.RecordingStream("rerun_example_spans", recording_id="episode_a") as rec:
        rec.save(path)
        rec.send_property("episode", rr.AnyValues(split="train"))
        rec.send_columns(
            "value",
            indexes=[rr.TimeColumn("time", duration=[0.0, 20.0])],
            columns=rr.Scalars.columns(scalars=[0.0, 1.0]),
        )
    dataset.register([path.as_uri()]).wait()
    return dataset


@pytest.mark.local_only
def test_reader_joins_segment_properties(spans_dataset: DatasetEntry, request: pytest.FixtureRequest) -> None:
    spans = SpanTable.create(spans_dataset, index="time")
    request.addfinalizer(spans.delete)
    spans.append(segment_id="episode_a", start=timedelta(seconds=1), end=timedelta(seconds=2))

    assert spans.reader().to_arrow_table()["property:episode:split"].to_pylist() == [["train"]]


@pytest.mark.local_only
def test_span_tables_are_separate_per_dataset(
    spans_dataset: DatasetEntry, entry_factory: EntryFactory, tmp_path: Path, request: pytest.FixtureRequest
) -> None:
    other_dataset = entry_factory.create_dataset("other_recordings")
    other_dataset.register([(tmp_path / "episode_a.rrd").as_uri()]).wait()
    spans = SpanTable.create(spans_dataset, index="time")
    request.addfinalizer(spans.delete)
    other = SpanTable.create(other_dataset, index="time")
    request.addfinalizer(other.delete)

    assert other.table.name != spans.table.name
    assert SpanTable.open(other_dataset).table.id == other.table.id


@pytest.mark.local_only
def test_open_finds_table_after_dataset_rename(spans_dataset: DatasetEntry, request: pytest.FixtureRequest) -> None:
    spans = SpanTable.create(spans_dataset, index="time")
    request.addfinalizer(spans.delete)

    spans_dataset.set_name("renamed_recordings")
    renamed = spans_dataset.catalog.get_dataset(id=spans_dataset.id)

    assert SpanTable.open(renamed).table.id == spans.table.id


@pytest.mark.local_only
def test_span_table_outlives_dataset_deletion(spans_dataset: DatasetEntry, request: pytest.FixtureRequest) -> None:
    spans = SpanTable.create(spans_dataset, index="time")
    request.addfinalizer(spans.delete)

    spans_dataset.delete()

    assert spans_dataset.catalog.get_table(id=spans.table.id).id == spans.table.id

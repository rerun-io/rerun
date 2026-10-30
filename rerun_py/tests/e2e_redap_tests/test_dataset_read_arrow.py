"""Tests for `DatasetSelection._read_arrow`."""

from __future__ import annotations

import time
from typing import TYPE_CHECKING, Any

import numpy as np
import pyarrow as pa
import pytest
import rerun as rr
from rerun.experimental import query_metrics

if TYPE_CHECKING:
    from pathlib import Path

    from rerun.catalog import DatasetEntry, DatasetSelection

    from e2e_redap_tests.conftest import EntryFactory


def read_both(selection: DatasetSelection, **kwargs: Any) -> tuple[pa.Table, pa.Table]:
    """Return `(read_arrow, reader)` results for the same query."""

    reader = selection._read_arrow(**kwargs)
    assert isinstance(reader, pa.RecordBatchReader)

    return reader.read_all(), pa.table(selection.reader(**kwargs))


def assert_same_rows(actual: pa.Table, expected: pa.Table) -> None:
    """Compare as multisets, since the order of rows with equal keys is unspecified."""

    assert actual.schema.equals(expected.schema, check_metadata=True)
    assert sorted(map(repr, actual.to_pylist())) == sorted(map(repr, expected.to_pylist()))


def assert_ordered(table: pa.Table, index: str | None) -> None:
    """Assert `(rerun_segment_id ASC, index ASC NULLS FIRST)` order."""

    segments = table["rerun_segment_id"].to_pylist()
    if index is None:
        assert segments == sorted(segments)
        return

    keys = [
        (segment, (0,) if value is None else (1, value))
        for segment, value in zip(segments, table[index].to_pylist(), strict=True)
    ]
    assert keys == sorted(keys)


def test_read_arrow_orders_by_segment_then_index(readonly_test_dataset: DatasetEntry) -> None:
    # The segments' `time_1` ranges overlap, so a global time sort would interleave them.
    selection = readonly_test_dataset.filter_segments(readonly_test_dataset.segment_ids())

    actual, expected = read_both(selection, index="time_1")

    assert len(set(actual["rerun_segment_id"].to_pylist())) > 1
    assert_ordered(actual, "time_1")
    assert_same_rows(actual, expected)


def scalar_dataset_selection(
    entry_factory: EntryFactory, tmp_path: Path, name: str, num_segments: int, rows_per_segment: int
) -> DatasetSelection:
    """Register `num_segments` segments of `rows_per_segment` scalars on the `frame` index, and select them all."""

    uris = []
    for i in range(num_segments):
        rrd_path = tmp_path / f"{name}_{i}.rrd"
        with rr.RecordingStream(f"rerun_example_{name}", recording_id=f"segment_{i}") as rec:
            rec.save(rrd_path)
            frames = np.arange(rows_per_segment)
            rec.send_columns(
                "scalar",
                indexes=[rr.TimeColumn("frame", sequence=frames)],
                columns=rr.Scalars.columns(scalars=frames * num_segments + i),
            )
            rec.flush()
        uris.append(rrd_path.absolute().as_uri())

    dataset = entry_factory.create_dataset(name)
    dataset.register(uris).wait()
    return dataset.filter_segments(dataset.segment_ids())


@pytest.mark.local_only
def test_read_arrow_orders_across_batches(entry_factory: EntryFactory, tmp_path: Path) -> None:
    # Enough rows that the merge emits several batches (DataFusion's default batch size is 8192).
    num_segments = 4
    rows_per_segment = 5000
    selection = scalar_dataset_selection(entry_factory, tmp_path, "read_arrow_batches", num_segments, rows_per_segment)

    reader = selection._read_arrow(index="frame")
    batches = list(reader)
    actual = pa.Table.from_batches(batches, schema=reader.schema)

    assert len(batches) > 1
    assert actual.num_rows == num_segments * rows_per_segment
    assert_ordered(actual, "frame")
    assert_same_rows(actual, pa.table(selection.reader(index="frame")))


@pytest.mark.local_only
def test_read_arrow_close_mid_stream_stops_the_query(entry_factory: EntryFactory, tmp_path: Path) -> None:
    # The segment must be far larger than what the merge and the pipeline buffer, so that the query cannot finish
    # before the reader is closed and the metrics snapshot can only come from dropping the stream.
    num_rows = 200_000
    selection = scalar_dataset_selection(entry_factory, tmp_path, "read_arrow_close", 1, num_rows)

    with query_metrics() as metrics:
        reader = selection._read_arrow(index="frame")
        assert reader.read_next_batch().num_rows > 0
        assert metrics.last_query() is None, "the query finished before the reader was closed"

        reader.close()

        # Dropping aborts the pipeline's tasks on the tokio runtime, so the snapshot can land shortly after `close`.
        deadline = time.monotonic() + 10.0
        while metrics.last_query() is None and time.monotonic() < deadline:
            time.sleep(0.05)

        snapshot = metrics.last_query()
        assert snapshot is not None, "closing the reader did not stop the query"
        assert snapshot.delivered_rows < num_rows, (
            f"the query delivered every row ({snapshot.delivered_rows}), so it was not stopped early"
        )


@pytest.mark.local_only
def test_read_arrow_into_table(entry_factory: EntryFactory, tmp_path: Path) -> None:
    # `TableEntry.append` pulls the reader from a tokio worker thread.
    selection = scalar_dataset_selection(entry_factory, tmp_path, "read_arrow_into_table", 2, 1000)

    reader = selection._read_arrow(index="frame")
    table = entry_factory.create_table("read_arrow_table", reader.schema, (tmp_path / "table").as_uri())
    table.append(reader)

    assert_same_rows(pa.table(table.reader()), selection._read_arrow(index="frame").read_all())


def test_read_arrow_static_only(readonly_test_dataset: DatasetEntry) -> None:
    selection = readonly_test_dataset.filter_segments(readonly_test_dataset.segment_ids())

    actual, expected = read_both(selection, index=None)

    assert actual.num_rows > 0
    assert_ordered(actual, None)
    assert_same_rows(actual, expected)


def test_read_arrow_filter_contents(readonly_test_dataset: DatasetEntry) -> None:
    selection = readonly_test_dataset.filter_segments(readonly_test_dataset.segment_ids()).filter_contents(["/obj1/**"])

    actual, expected = read_both(selection, index="time_1")

    assert actual.num_rows > 0
    assert_ordered(actual, "time_1")
    assert_same_rows(actual, expected)


def test_read_arrow_using_index_values(readonly_test_dataset: DatasetEntry) -> None:
    selection = readonly_test_dataset.filter_segments([
        "3ee345b2e801448cace33a1097b9b49b",
        "68224eead5ed40838b3f3bdb0edfd2b2",
    ])
    using_index_values = {
        "3ee345b2e801448cace33a1097b9b49b": np.array(
            [
                np.datetime64("2024-01-15T10:44:45.123456789", "ns"),
                np.datetime64("2024-01-15T10:34:45.123456789", "ns"),
            ],
            dtype=np.datetime64,
        ),
        "68224eead5ed40838b3f3bdb0edfd2b2": np.array(
            [np.datetime64("2024-01-15T10:40:45.123456789", "ns")],
            dtype=np.datetime64,
        ),
    }

    for fill_latest_at in [False, True]:
        actual, expected = read_both(
            selection,
            index="time_1",
            using_index_values=using_index_values,
            fill_latest_at=fill_latest_at,
        )

        assert actual.num_rows == 3
        assert_ordered(actual, "time_1")
        assert_same_rows(actual, expected)


def test_read_arrow_empty_selection(readonly_test_dataset: DatasetEntry) -> None:
    known = min(readonly_test_dataset.segment_ids())
    full_schema = readonly_test_dataset.filter_segments([known])._read_arrow(index="time_1").schema

    for empty in [
        readonly_test_dataset.filter_segments([]),
        readonly_test_dataset.filter_segments([known]).filter_segments(["does-not-exist"]),
    ]:
        table = empty._read_arrow(index="time_1").read_all()

        assert table.num_rows == 0
        assert table.schema.equals(full_schema, check_metadata=True)


def test_read_arrow_is_single_pass(readonly_test_dataset: DatasetEntry) -> None:
    known = min(readonly_test_dataset.segment_ids())
    reader = readonly_test_dataset.filter_segments([known])._read_arrow(index="time_1")

    first = reader.read_all()
    second = reader.read_all()

    assert first.num_rows > 0
    assert second.num_rows == 0
    assert second.schema.equals(first.schema, check_metadata=True)

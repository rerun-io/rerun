"""Tests for the DatasetView API."""

from __future__ import annotations

import datetime
from typing import TYPE_CHECKING

import datafusion as dfn
import numpy as np
import pyarrow as pa
import pytest
from inline_snapshot import snapshot as inline_snapshot

if TYPE_CHECKING:
    from pathlib import Path

    import datafusion
    from pytest import LogCaptureFixture
    from rerun.catalog import DatasetEntry
    from syrupy import SnapshotAssertion

    from e2e_redap_tests.conftest import EntryFactory


def test_dataset_view_filter_segments(readonly_test_dataset: DatasetEntry) -> None:
    """Test filtering a dataset by segment IDs."""

    # Get actual segment IDs from the dataset
    all_segments = sorted(readonly_test_dataset.segment_ids())
    assert len(all_segments) >= 2, "Need at least 2 segments for this test"

    # Simple filter by segment list - pick first two segments
    filter_segments = all_segments[:2]
    view = readonly_test_dataset.filter_segments(filter_segments)
    assert sorted(view.segment_ids()) == filter_segments


@pytest.mark.local_only
@pytest.mark.creates_table
def test_dataset_view_filter_segments_with_dataframe(
    readonly_test_dataset: DatasetEntry, entry_factory: EntryFactory, tmp_path: Path
) -> None:
    """Test filtering a dataset using a metadata table."""

    # Get actual segment IDs from the dataset
    all_segments = sorted(readonly_test_dataset.segment_ids())
    assert len(all_segments) >= 3, "Need at least 5 segments for this test"

    # Use first 5 segments for the metadata table
    test_segments = all_segments[:3]

    # Create metadata table with success indicators
    segments = entry_factory.create_table(
        "metadata",
        pa.schema([
            ("rerun_segment_id", pa.string()),
        ]),
        tmp_path.as_uri(),
    )
    segments.append(
        rerun_segment_id=test_segments,
    )

    # Filter to only successful segments using DataFrame
    view = readonly_test_dataset.filter_segments(segments.reader())

    assert set(view.segment_ids()) == set(test_segments)
    assert view.segment_table().count() == len(test_segments)


def sort_schema(schema: pa.Schema) -> str:
    """Sort schema fields by name for order-independent comparison."""

    all_fields = {}
    for field in schema:
        # Sorted field metadata
        field_meta = {}
        if field.metadata is not None:
            for k, v in field.metadata.items():
                field_meta[k] = v
        all_fields[field.name] = dict(sorted(field_meta.items(), key=lambda item: item[0]))

    sorted_fields = dict(sorted(all_fields.items(), key=lambda item: item[0]))

    output = ""
    for name, field_meta in sorted_fields.items():
        output += f"{name}:\n"
        for k, v in field_meta.items():
            output += f"    {k}: {v}\n"

    return output


def test_dataset_view_filter_contents(readonly_test_dataset: DatasetEntry, snapshot: SnapshotAssertion) -> None:
    """Test filtering a dataset by entity paths."""

    schema = sort_schema(pa.schema(readonly_test_dataset.schema()))
    assert str(schema) == snapshot()

    view = readonly_test_dataset.filter_contents(["/obj1/**"])
    schema = sort_schema(pa.schema(view.schema()))
    assert str(schema) == snapshot()


def test_dataset_view_filter_contents_nonexistent_path(
    readonly_test_dataset: DatasetEntry, snapshot: SnapshotAssertion
) -> None:
    """Test that filtering by a non-existent path returns no data columns."""

    # Filter by a path that doesn't exist
    view = readonly_test_dataset.filter_contents(["/this/does/not/exist/**"])
    schema = sort_schema(pa.schema(view.schema()))
    assert str(schema) == snapshot()

    df = view.reader(index="time_1")
    schema = sort_schema(pa.schema(df.schema()))
    assert str(schema) == snapshot()


def test_dataset_view_filter_contents_empty_list(
    readonly_test_dataset: DatasetEntry, snapshot: SnapshotAssertion
) -> None:
    """Test that filtering with an empty list returns no data columns."""

    view = readonly_test_dataset.filter_contents([])

    schema = sort_schema(pa.schema(view.schema()))
    assert str(schema) == snapshot()

    df = view.reader(index="time_1")
    schema = sort_schema(pa.schema(df.schema()))
    assert str(schema) == snapshot()


def test_dataset_view_no_filter_contents(readonly_test_dataset: DatasetEntry, snapshot: SnapshotAssertion) -> None:
    """Test that not using filter_contents returns all data columns."""

    all_segments = sorted(readonly_test_dataset.segment_ids())
    view = readonly_test_dataset.filter_segments(all_segments[:1])

    # Schema should include all data columns
    schema = sort_schema(pa.schema(view.schema()))
    assert str(schema) == snapshot()

    df = view.reader(index="time_1")
    schema = sort_schema(pa.schema(df.schema()))
    assert str(schema) == snapshot()


def test_dataset_view_reader(readonly_test_dataset: DatasetEntry, snapshot: SnapshotAssertion) -> None:
    """Test reading data through a DatasetView."""

    # Get first segment ID
    all_segments = sorted(readonly_test_dataset.segment_ids())
    assert len(all_segments) >= 1, "Need at least 1 segment for this test"

    first_segment = all_segments[0]

    view = readonly_test_dataset.filter_segments([first_segment]).filter_contents(["/obj1/**"])
    df = view.reader(index="time_1")

    df = sorted_df(df)

    assert str(df) == snapshot()


def sorted_df(df: datafusion.DataFrame) -> datafusion.DataFrame:
    sorted_fields = sorted([field.name for field in df.schema()])
    return df.select(*sorted_fields)


def test_dataframe_api_using_index_values(readonly_test_dataset: DatasetEntry, snapshot: SnapshotAssertion) -> None:
    dataset_view = readonly_test_dataset.filter_segments([
        "3ee345b2e801448cace33a1097b9b49b",
        "68224eead5ed40838b3f3bdb0edfd2b2",
    ])

    df = (
        dataset_view.reader(
            index="time_1",
        )
        .select("rerun_segment_id", "time_1", "/obj1:Points3D:positions", "/obj2:Points3D:positions")
        .sort("rerun_segment_id", "time_1")
    )

    assert str(df) == snapshot

    # Create a view with all partitions
    df = (
        dataset_view.reader(
            index="time_1",
            using_index_values={
                "3ee345b2e801448cace33a1097b9b49b": np.array(
                    [
                        np.datetime64("2024-01-15T10:34:45.123456789", "ns"),
                        np.datetime64("2024-01-15T10:44:45.123456789", "ns"),
                    ],
                    dtype=np.datetime64,
                ),
                "68224eead5ed40838b3f3bdb0edfd2b2": np.array(
                    [
                        np.datetime64("2024-01-15T10:40:45.123456789", "ns"),
                    ],
                    dtype=np.datetime64,
                ),
            },
            fill_latest_at=False,
        )
        .select("rerun_segment_id", "time_1", "/obj1:Points3D:positions", "/obj2:Points3D:positions")
        .sort("rerun_segment_id", "time_1")
    )

    assert str(df) == snapshot

    assert str(pa.table(df)) == snapshot


def test_dataframe_api_using_index_values_same_indices_on_all_segments(
    readonly_test_dataset: DatasetEntry, snapshot: SnapshotAssertion
) -> None:
    dataset_view = readonly_test_dataset.filter_segments([
        "3ee345b2e801448cace33a1097b9b49b",
        "68224eead5ed40838b3f3bdb0edfd2b2",
    ])

    # Create a view with all partitions
    df = (
        dataset_view.reader(
            index="time_1",
            using_index_values=np.array(
                [
                    np.datetime64("2024-01-15T10:34:45.123456789", "ns"),
                    np.datetime64("2024-01-15T10:44:45.123456789", "ns"),
                ],
                dtype=np.datetime64,
            ),
            fill_latest_at=False,
        )
        .select("rerun_segment_id", "time_1", "/obj1:Points3D:positions", "/obj2:Points3D:positions")
        .sort("rerun_segment_id", "time_1")
    )

    assert str(df) == snapshot

    assert str(pa.table(df)) == snapshot


def test_dataframe_api_using_index_values_empty(
    readonly_test_dataset: DatasetEntry, caplog: LogCaptureFixture, snapshot: SnapshotAssertion
) -> None:
    df = readonly_test_dataset.reader(
        index="time_1",
        using_index_values={
            "doesnt_exist": np.array(
                [
                    datetime.datetime(1999, 12, 31, 23, 59, 59),
                    datetime.datetime(2000, 1, 1, 0, 0, 1, microsecond=500),
                ],
                dtype=np.datetime64,
            ),
            "f5e43eb07b11431386f4d5bf8833de30": np.array([], dtype=np.datetime64),
        },
        fill_latest_at=True,
    ).select(
        "rerun_segment_id",
        "time_1",
        "time_2",
        "time_3",
        "/obj1:Points3D:positions",
        "/obj2:Points3D:positions",
        "/obj3:Points3D:positions",
        "/text1:TextDocument:text",
        "/text2:TextDocument:text",
    )

    assert len(caplog.records) == 1
    assert caplog.records[0].msg == inline_snapshot(
        "Index values for the following inexistent or filtered segments were ignored: doesnt_exist"
    )

    assert str(df) == inline_snapshot("No data to display")

    assert str(pa.table(df)) == snapshot


def test_dataframe_api_using_index_values_dataframe(
    readonly_test_dataset: DatasetEntry, snapshot: SnapshotAssertion
) -> None:
    """Demonstrate using the output of one query as `using_index_values` input for another."""

    # TODO(ab, jleibs): this example is slightly unfortunate because it is more about filtering rows than
    # interpolating rows.

    rows_of_interest = (
        readonly_test_dataset.filter_contents(["/obj1/**"])
        .reader(index="time_1")
        .filter(dfn.col("/obj1:Points3D:positions").is_not_null())
    )

    df = (
        readonly_test_dataset.filter_contents(["/obj2/**"])
        .reader(index="time_1", using_index_values=rows_of_interest)
        .select("rerun_segment_id", "time_1", "/obj2:Points3D:positions")
        .sort("rerun_segment_id", "time_1")
    )

    assert str(df) == snapshot

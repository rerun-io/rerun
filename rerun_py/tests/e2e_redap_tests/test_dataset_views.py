"""Tests for the DatasetView API."""

from __future__ import annotations

import datetime
from typing import TYPE_CHECKING

import datafusion as dfn
import numpy as np
import pyarrow as pa
import pytest
from inline_snapshot import snapshot as inline_snapshot
from rerun.catalog import ContentFilter

if TYPE_CHECKING:
    from pathlib import Path

    import datafusion
    from pytest import LogCaptureFixture
    from rerun.catalog import DatasetEntry, IndexValuesLike
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
    df_schema = df.schema()
    for batch in df.collect():
        assert batch.schema.equals(df_schema, check_metadata=True)

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
        dataset_view
        .reader(
            index="time_1",
        )
        .select("rerun_segment_id", "time_1", "/obj1:Points3D:positions", "/obj2:Points3D:positions")
        .sort("rerun_segment_id", "time_1")
    )

    df_schema = df.schema()
    for batch in df.collect():
        assert batch.schema.equals(df_schema, check_metadata=True)
    assert str(df) == snapshot

    # Create a view with all partitions
    df = (
        dataset_view
        .reader(
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

    df_schema = df.schema()
    for batch in df.collect():
        assert batch.schema.equals(df_schema, check_metadata=True)
    assert str(df) == snapshot

    assert str(pa.table(df)) == snapshot


def test_dataframe_api_using_index_values_same_indices_on_all_segments(
    readonly_test_dataset: DatasetEntry, snapshot: SnapshotAssertion
) -> None:
    dataset_view = readonly_test_dataset.filter_segments([
        "3ee345b2e801448cace33a1097b9b49b",
        "68224eead5ed40838b3f3bdb0edfd2b2",
    ])

    all_index_values: list[IndexValuesLike] = [
        pa.array([1705314885123456789, 1705315485123456789], type=pa.int64()),
        pa.chunked_array([[1705314885123456789], [1705315485123456789]], type=pa.int64()),
        np.array([np.int64(1705314885123456789), np.int64(1705315485123456789)], dtype=np.int64),
        np.array(
            [
                np.datetime64("2024-01-15T10:34:45.123456789", "ns"),
                np.datetime64("2024-01-15T10:44:45.123456789", "ns"),
            ],
            dtype=np.datetime64,
        ),
        pa.array([1705314885123456789, 1705315485123456789], type=pa.timestamp("ns")),
    ]

    results = []
    arrow_results = []
    for index_values in all_index_values:
        df = (
            dataset_view
            .reader(
                index="time_1",
                using_index_values=index_values,
                fill_latest_at=False,
            )
            .select("rerun_segment_id", "time_1", "/obj1:Points3D:positions", "/obj2:Points3D:positions")
            .sort("rerun_segment_id", "time_1")
        )
        results.append(str(df))
        arrow_results.append(str(pa.table(df)))

        df_schema = df.schema()
        for batch in df.collect():
            assert batch.schema.equals(df_schema, check_metadata=True)

    # All input types must produce identical results
    for r in results[1:]:
        assert r == results[0]
    for r in arrow_results[1:]:
        assert r == arrow_results[0]

    # Snapshot once per representation
    assert results[0] == snapshot
    assert arrow_results[0] == snapshot

    # Verify Python datetime.datetime objects work via numpy conversion.
    # Tested separately because datetime.datetime only has microsecond precision,
    # so the resulting index values differ from the nanosecond-precise ones above.
    datetime_index_values = np.array(
        [
            datetime.datetime(2024, 1, 15, 10, 34, 45, 123456),
            datetime.datetime(2024, 1, 15, 10, 44, 45, 123456),
        ],
        dtype="datetime64[ns]",
    )
    df = (
        dataset_view
        .reader(
            index="time_1",
            using_index_values=datetime_index_values,
            fill_latest_at=False,
        )
        .select("rerun_segment_id", "time_1", "/obj1:Points3D:positions", "/obj2:Points3D:positions")
        .sort("rerun_segment_id", "time_1")
    )
    table = pa.table(df)
    assert table.num_rows > 0, "Expected results when using datetime.datetime index values"


def test_dataframe_api_using_index_values_partial_overlap(
    readonly_test_dataset: DatasetEntry, snapshot: SnapshotAssertion
) -> None:
    """
    Test that index values are restricted to segments whose range covers them.

    Uses two segments with different time_1 ranges:
    - 3ee345b2 spans 2024-01-15T10:30:45 to 2024-01-15T11:18:45
    - 141a866d spans 2024-01-15T10:30:45 to 2024-01-15T12:04:45

    We request three timestamps:
    - 10:34:45 — inside both segments
    - 11:30:45 — inside 141a866d only (after 3ee345b2's end)
    - 12:30:00 — after both segments

    Expected: 3 rows total (2 for 141a866d, 1 for 3ee345b2).
    """
    dataset_view = readonly_test_dataset.filter_segments([
        "3ee345b2e801448cace33a1097b9b49b",
        "141a866deb2d49f69eb3215e8a404ffc",
    ])

    df = (
        dataset_view
        .reader(
            index="time_1",
            using_index_values=np.array(
                [
                    np.datetime64("2024-01-15T10:34:45.123456789", "ns"),  # inside both
                    np.datetime64("2024-01-15T11:30:45.123456789", "ns"),  # inside 141a only
                    np.datetime64("2024-01-15T12:30:00.000000000", "ns"),  # after both
                ],
                dtype=np.datetime64,
            ),
            fill_latest_at=True,
        )
        .select("rerun_segment_id", "time_1", "/obj1:Points3D:positions", "/obj2:Points3D:positions")
        .sort("rerun_segment_id", "time_1")
    )

    table = pa.table(df)
    seg_col = table.column("rerun_segment_id").to_pylist()
    time_col = table.column("time_1").cast(pa.int64()).to_pylist()

    in_both_ns = 1705314885123456789  # 2024-01-15T10:34:45.123456789
    in_141a_only_ns = 1705318245123456789  # 2024-01-15T11:30:45.123456789

    # Both segments should have the shared timestamp
    assert seg_col.count("3ee345b2e801448cace33a1097b9b49b") == 1
    assert seg_col.count("141a866deb2d49f69eb3215e8a404ffc") == 2

    # 3ee345b2 should only have the in-both timestamp
    for seg, ts in zip(seg_col, time_col, strict=False):
        if seg == "3ee345b2e801448cace33a1097b9b49b":
            assert ts == in_both_ns
        elif seg == "141a866deb2d49f69eb3215e8a404ffc":
            assert ts in (in_both_ns, in_141a_only_ns)

    # The after-both timestamp (12:30:00) should not appear at all
    after_both_ns = 1705321800000000000  # 2024-01-15T12:30:00
    assert after_both_ns not in time_col

    assert str(df) == snapshot
    assert str(table) == snapshot


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
        readonly_test_dataset
        .filter_contents(["/obj1/**"])
        .reader(index="time_1")
        .filter(dfn.col("/obj1:Points3D:positions").is_not_null())
    )

    df = (
        readonly_test_dataset
        .filter_contents(["/obj2/**"])
        .reader(index="time_1", using_index_values=rows_of_interest)
        .select("rerun_segment_id", "time_1", "/obj2:Points3D:positions")
        .sort("rerun_segment_id", "time_1")
    )

    df_schema = df.schema()
    for batch in df.collect():
        assert batch.schema.equals(df_schema, check_metadata=True)
    assert str(df) == snapshot


@pytest.mark.parametrize(
    "arrow_type",
    [
        pa.int64(),
        pa.timestamp("ns"),
        pa.duration("ns"),
    ],
)
def test_using_index_values_with_arrow_types(readonly_test_dataset: DatasetEntry, arrow_type: pa.DataType) -> None:
    """Regression test for RR-3905. All arrow types returned by get_index_ranges() should be accepted."""

    values = pa.array([1_000_000_000, 2_000_000_000], type=arrow_type)

    # This should not fail
    readonly_test_dataset.reader(
        index="time_1",
        using_index_values=values,
    )


def test_content_filter_everything_matches_glob(readonly_test_dataset: DatasetEntry) -> None:
    """ContentFilter.everything() should produce the same schema as filter_contents('/**')."""
    raw = sort_schema(pa.schema(readonly_test_dataset.schema()))
    builder = sort_schema(pa.schema(readonly_test_dataset.filter_contents(ContentFilter.everything()).schema()))
    assert builder == raw


def test_content_filter_nothing_matches_empty_list(readonly_test_dataset: DatasetEntry) -> None:
    """ContentFilter.nothing() should produce the same schema as filter_contents([])."""
    raw = sort_schema(pa.schema(readonly_test_dataset.filter_contents([]).schema()))
    builder = sort_schema(pa.schema(readonly_test_dataset.filter_contents(ContentFilter.nothing()).schema()))
    assert builder == raw


def test_content_filter_include_subtree_matches_raw(readonly_test_dataset: DatasetEntry) -> None:
    """ContentFilter include with subtree=True should match the equivalent raw expression."""
    raw = sort_schema(pa.schema(readonly_test_dataset.filter_contents(["/obj1/**"]).schema()))
    builder = sort_schema(
        pa.schema(
            readonly_test_dataset.filter_contents(ContentFilter.nothing().include("/obj1", subtree=True)).schema()
        )
    )
    assert builder == raw


def test_content_filter_exclude_matches_raw(readonly_test_dataset: DatasetEntry, snapshot: SnapshotAssertion) -> None:
    """ContentFilter with include + exclude should match the equivalent raw expression list."""
    raw_exprs = ["/obj1/**", "/obj2/**", "-/obj2/**"]
    raw = sort_schema(pa.schema(readonly_test_dataset.filter_contents(raw_exprs).schema()))

    builder = sort_schema(
        pa.schema(
            readonly_test_dataset.filter_contents(
                ContentFilter
                .nothing()
                .include("/obj1", subtree=True)
                .include("/obj2", subtree=True)
                .exclude("/obj2", subtree=True)
            ).schema()
        )
    )
    assert builder == raw
    assert builder == snapshot()


def test_content_filter_on_dataset_view(readonly_test_dataset: DatasetEntry) -> None:
    """ContentFilter should work when called on a DatasetView (chained from filter_segments)."""
    all_segments = sorted(readonly_test_dataset.segment_ids())
    assert len(all_segments) >= 1

    view = readonly_test_dataset.filter_segments(all_segments[:1])

    raw = sort_schema(pa.schema(view.filter_contents(["/obj1/**"]).schema()))
    builder = sort_schema(
        pa.schema(view.filter_contents(ContentFilter.nothing().include("/obj1", subtree=True)).schema())
    )
    assert builder == raw

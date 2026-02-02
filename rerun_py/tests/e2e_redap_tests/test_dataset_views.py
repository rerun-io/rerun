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


def test_dataframe_api_using_index_values(readonly_test_dataset: DatasetEntry) -> None:
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

    assert str(df) == inline_snapshot("""\
┌──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                                                                                                        │
│ * version: 0.1.2                                                                                                                                                                 │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌──────────────────────────────────┬───────────────────────────────┬─────────────────────────────────────────────────────┬─────────────────────────────────────────────────────┐ │
│ │ rerun_segment_id                 ┆ time_1                        ┆ /obj1:Points3D:positions                            ┆ /obj2:Points3D:positions                            │ │
│ │ ---                              ┆ ---                           ┆ ---                                                 ┆ ---                                                 │ │
│ │ type: Utf8                       ┆ type: nullable Timestamp(ns)  ┆ type: nullable List[nullable FixedSizeList[f32; 3]] ┆ type: nullable List[nullable FixedSizeList[f32; 3]] │ │
│ │                                  ┆ index_name: time_1            ┆ archetype: Points3D                                 ┆ archetype: Points3D                                 │ │
│ │                                  ┆ kind: index                   ┆ component: Points3D:positions                       ┆ component: Points3D:positions                       │ │
│ │                                  ┆                               ┆ component_type: Position3D                          ┆ component_type: Position3D                          │ │
│ │                                  ┆                               ┆ entity_path: /obj1                                  ┆ entity_path: /obj2                                  │ │
│ │                                  ┆                               ┆ kind: data                                          ┆ kind: data                                          │ │
│ ╞══════════════════════════════════╪═══════════════════════════════╪═════════════════════════════════════════════════════╪═════════════════════════════════════════════════════╡ │
│ │ 3ee345b2e801448cace33a1097b9b49b ┆ 2024-01-15T10:30:45.123456789 ┆ null                                                ┆ [[19.0, 1.0, 0.0]]                                  │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ 3ee345b2e801448cace33a1097b9b49b ┆ 2024-01-15T10:34:45.123456789 ┆ [[3.0, 0.0, 0.0]]                                   ┆ [[13.0, 1.0, 0.0]]                                  │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ 3ee345b2e801448cace33a1097b9b49b ┆ 2024-01-15T10:36:45.123456789 ┆ [[4.0, 0.0, 0.0]]                                   ┆ [[1.0, 1.0, 0.0]]                                   │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ 3ee345b2e801448cace33a1097b9b49b ┆ 2024-01-15T10:38:45.123456789 ┆ [[5.0, 0.0, 0.0]]                                   ┆ [[5.0, 1.0, 0.0]]                                   │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ 3ee345b2e801448cace33a1097b9b49b ┆ 2024-01-15T10:40:45.123456789 ┆ [[6.0, 0.0, 0.0]]                                   ┆ [[25.0, 1.0, 0.0]]                                  │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ 3ee345b2e801448cace33a1097b9b49b ┆ 2024-01-15T10:44:45.123456789 ┆ null                                                ┆ null                                                │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ 3ee345b2e801448cace33a1097b9b49b ┆ 2024-01-15T10:46:45.123456789 ┆ null                                                ┆ [[22.0, 1.0, 0.0]]                                  │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ 3ee345b2e801448cace33a1097b9b49b ┆ 2024-01-15T10:48:45.123456789 ┆ null                                                ┆ [[9.0, 1.0, 0.0]]                                   │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ 3ee345b2e801448cace33a1097b9b49b ┆ 2024-01-15T10:50:45.123456789 ┆ null                                                ┆ [[2.0, 1.0, 0.0]]                                   │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ 3ee345b2e801448cace33a1097b9b49b ┆ 2024-01-15T10:52:45.123456789 ┆ [[12.0, 0.0, 0.0]]                                  ┆ [[4.0, 1.0, 0.0]]                                   │ │
│ └──────────────────────────────────┴───────────────────────────────┴─────────────────────────────────────────────────────┴─────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
Data truncated due to size.\
""")

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

    assert str(df) == inline_snapshot("""\
┌──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                                                                                                        │
│ * version: 0.1.2                                                                                                                                                                 │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌──────────────────────────────────┬───────────────────────────────┬─────────────────────────────────────────────────────┬─────────────────────────────────────────────────────┐ │
│ │ rerun_segment_id                 ┆ time_1                        ┆ /obj1:Points3D:positions                            ┆ /obj2:Points3D:positions                            │ │
│ │ ---                              ┆ ---                           ┆ ---                                                 ┆ ---                                                 │ │
│ │ type: Utf8                       ┆ type: nullable Timestamp(ns)  ┆ type: nullable List[nullable FixedSizeList[f32; 3]] ┆ type: nullable List[nullable FixedSizeList[f32; 3]] │ │
│ │                                  ┆ index_name: time_1            ┆ archetype: Points3D                                 ┆ archetype: Points3D                                 │ │
│ │                                  ┆ kind: index                   ┆ component: Points3D:positions                       ┆ component: Points3D:positions                       │ │
│ │                                  ┆                               ┆ component_type: Position3D                          ┆ component_type: Position3D                          │ │
│ │                                  ┆                               ┆ entity_path: /obj1                                  ┆ entity_path: /obj2                                  │ │
│ │                                  ┆                               ┆ kind: data                                          ┆ kind: data                                          │ │
│ ╞══════════════════════════════════╪═══════════════════════════════╪═════════════════════════════════════════════════════╪═════════════════════════════════════════════════════╡ │
│ │ 3ee345b2e801448cace33a1097b9b49b ┆ 2024-01-15T10:34:45.123456789 ┆ [[3.0, 0.0, 0.0]]                                   ┆ [[13.0, 1.0, 0.0]]                                  │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ 3ee345b2e801448cace33a1097b9b49b ┆ 2024-01-15T10:44:45.123456789 ┆ null                                                ┆ null                                                │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ 68224eead5ed40838b3f3bdb0edfd2b2 ┆ 2024-01-15T10:40:45.123456789 ┆ [[6.0, 0.0, 0.0]]                                   ┆ null                                                │ │
│ └──────────────────────────────────┴───────────────────────────────┴─────────────────────────────────────────────────────┴─────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘\
""")

    assert str(pa.table(df)) == inline_snapshot("""\
pyarrow.Table
rerun_segment_id: string not null
time_1: timestamp[ns]
/obj1:Points3D:positions: list<item: fixed_size_list<item: float not null>[3]>
  child 0, item: fixed_size_list<item: float not null>[3]
      child 0, item: float not null
/obj2:Points3D:positions: list<item: fixed_size_list<item: float not null>[3]>
  child 0, item: fixed_size_list<item: float not null>[3]
      child 0, item: float not null
----
rerun_segment_id: [["3ee345b2e801448cace33a1097b9b49b","3ee345b2e801448cace33a1097b9b49b","68224eead5ed40838b3f3bdb0edfd2b2"]]
time_1: [[2024-01-15 10:34:45.123456789,2024-01-15 10:44:45.123456789,2024-01-15 10:40:45.123456789]]
/obj1:Points3D:positions: [[[[3,0,0]],null,[[6,0,0]]]]
/obj2:Points3D:positions: [[[[13,1,0]],null,null]]\
""")


def test_dataframe_api_using_index_values_same_indices_on_all_segments(readonly_test_dataset: DatasetEntry) -> None:
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

    assert str(df) == inline_snapshot("""\
┌──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                                                                                                        │
│ * version: 0.1.2                                                                                                                                                                 │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌──────────────────────────────────┬───────────────────────────────┬─────────────────────────────────────────────────────┬─────────────────────────────────────────────────────┐ │
│ │ rerun_segment_id                 ┆ time_1                        ┆ /obj1:Points3D:positions                            ┆ /obj2:Points3D:positions                            │ │
│ │ ---                              ┆ ---                           ┆ ---                                                 ┆ ---                                                 │ │
│ │ type: Utf8                       ┆ type: nullable Timestamp(ns)  ┆ type: nullable List[nullable FixedSizeList[f32; 3]] ┆ type: nullable List[nullable FixedSizeList[f32; 3]] │ │
│ │                                  ┆ index_name: time_1            ┆ archetype: Points3D                                 ┆ archetype: Points3D                                 │ │
│ │                                  ┆ kind: index                   ┆ component: Points3D:positions                       ┆ component: Points3D:positions                       │ │
│ │                                  ┆                               ┆ component_type: Position3D                          ┆ component_type: Position3D                          │ │
│ │                                  ┆                               ┆ entity_path: /obj1                                  ┆ entity_path: /obj2                                  │ │
│ │                                  ┆                               ┆ kind: data                                          ┆ kind: data                                          │ │
│ ╞══════════════════════════════════╪═══════════════════════════════╪═════════════════════════════════════════════════════╪═════════════════════════════════════════════════════╡ │
│ │ 3ee345b2e801448cace33a1097b9b49b ┆ 2024-01-15T10:34:45.123456789 ┆ [[3.0, 0.0, 0.0]]                                   ┆ [[13.0, 1.0, 0.0]]                                  │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ 3ee345b2e801448cace33a1097b9b49b ┆ 2024-01-15T10:44:45.123456789 ┆ null                                                ┆ null                                                │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ 68224eead5ed40838b3f3bdb0edfd2b2 ┆ 2024-01-15T10:34:45.123456789 ┆ [[3.0, 0.0, 0.0]]                                   ┆ null                                                │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ 68224eead5ed40838b3f3bdb0edfd2b2 ┆ 2024-01-15T10:44:45.123456789 ┆ [[8.0, 0.0, 0.0]]                                   ┆ [[5.0, 1.0, 0.0]]                                   │ │
│ └──────────────────────────────────┴───────────────────────────────┴─────────────────────────────────────────────────────┴─────────────────────────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘\
""")

    assert str(pa.table(df)) == inline_snapshot("""\
pyarrow.Table
rerun_segment_id: string not null
time_1: timestamp[ns]
/obj1:Points3D:positions: list<item: fixed_size_list<item: float not null>[3]>
  child 0, item: fixed_size_list<item: float not null>[3]
      child 0, item: float not null
/obj2:Points3D:positions: list<item: fixed_size_list<item: float not null>[3]>
  child 0, item: fixed_size_list<item: float not null>[3]
      child 0, item: float not null
----
rerun_segment_id: [["3ee345b2e801448cace33a1097b9b49b","3ee345b2e801448cace33a1097b9b49b","68224eead5ed40838b3f3bdb0edfd2b2","68224eead5ed40838b3f3bdb0edfd2b2"]]
time_1: [[2024-01-15 10:34:45.123456789,2024-01-15 10:44:45.123456789,2024-01-15 10:34:45.123456789,2024-01-15 10:44:45.123456789]]
/obj1:Points3D:positions: [[[[3,0,0]],null,[[3,0,0]],[[8,0,0]]]]
/obj2:Points3D:positions: [[[[13,1,0]],null,null,[[5,1,0]]]]\
""")


def test_dataframe_api_using_index_values_empty(readonly_test_dataset: DatasetEntry, caplog: LogCaptureFixture) -> None:
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

    assert str(pa.table(df)) == inline_snapshot("""\
pyarrow.Table
rerun_segment_id: string not null
time_1: timestamp[ns]
time_2: duration[ns]
time_3: int64
/obj1:Points3D:positions: list<item: fixed_size_list<item: float not null>[3]>
  child 0, item: fixed_size_list<item: float not null>[3]
      child 0, item: float not null
/obj2:Points3D:positions: list<item: fixed_size_list<item: float not null>[3]>
  child 0, item: fixed_size_list<item: float not null>[3]
      child 0, item: float not null
/obj3:Points3D:positions: list<item: fixed_size_list<item: float not null>[3]>
  child 0, item: fixed_size_list<item: float not null>[3]
      child 0, item: float not null
/text1:TextDocument:text: list<item: string>
  child 0, item: string
/text2:TextDocument:text: list<item: string>
  child 0, item: string
----
rerun_segment_id: []
time_1: []
time_2: []
time_3: []
/obj1:Points3D:positions: []
/obj2:Points3D:positions: []
/obj3:Points3D:positions: []
/text1:TextDocument:text: []
/text2:TextDocument:text: []\
""")


def test_dataframe_api_using_index_values_dataframe(readonly_test_dataset: DatasetEntry) -> None:
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

    assert str(df) == inline_snapshot("""\
┌────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                                                  │
│ * version: 0.1.2                                                                                                           │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌──────────────────────────────────┬───────────────────────────────┬─────────────────────────────────────────────────────┐ │
│ │ rerun_segment_id                 ┆ time_1                        ┆ /obj2:Points3D:positions                            │ │
│ │ ---                              ┆ ---                           ┆ ---                                                 │ │
│ │ type: Utf8                       ┆ type: nullable Timestamp(ns)  ┆ type: nullable List[nullable FixedSizeList[f32; 3]] │ │
│ │                                  ┆ index_name: time_1            ┆ archetype: Points3D                                 │ │
│ │                                  ┆ kind: index                   ┆ component: Points3D:positions                       │ │
│ │                                  ┆                               ┆ component_type: Position3D                          │ │
│ │                                  ┆                               ┆ entity_path: /obj2                                  │ │
│ │                                  ┆                               ┆ kind: data                                          │ │
│ ╞══════════════════════════════════╪═══════════════════════════════╪═════════════════════════════════════════════════════╡ │
│ │ 141a866deb2d49f69eb3215e8a404ffc ┆ 2024-01-15T10:30:45.123456789 ┆ [[38.0, 1.0, 0.0]]                                  │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ 141a866deb2d49f69eb3215e8a404ffc ┆ 2024-01-15T10:32:45.123456789 ┆ [[35.0, 1.0, 0.0]]                                  │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ 141a866deb2d49f69eb3215e8a404ffc ┆ 2024-01-15T10:34:45.123456789 ┆ null                                                │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ 141a866deb2d49f69eb3215e8a404ffc ┆ 2024-01-15T10:36:45.123456789 ┆ [[1.0, 1.0, 0.0]]                                   │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ 141a866deb2d49f69eb3215e8a404ffc ┆ 2024-01-15T10:38:45.123456789 ┆ [[9.0, 1.0, 0.0]]                                   │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ 141a866deb2d49f69eb3215e8a404ffc ┆ 2024-01-15T10:40:45.123456789 ┆ null                                                │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ 141a866deb2d49f69eb3215e8a404ffc ┆ 2024-01-15T10:42:45.123456789 ┆ [[33.0, 1.0, 0.0]]                                  │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ 141a866deb2d49f69eb3215e8a404ffc ┆ 2024-01-15T10:44:45.123456789 ┆ [[25.0, 1.0, 0.0]]                                  │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ 141a866deb2d49f69eb3215e8a404ffc ┆ 2024-01-15T10:48:45.123456789 ┆ null                                                │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ 141a866deb2d49f69eb3215e8a404ffc ┆ 2024-01-15T10:52:45.123456789 ┆ [[6.0, 1.0, 0.0]]                                   │ │
│ └──────────────────────────────────┴───────────────────────────────┴─────────────────────────────────────────────────────┘ │
└────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘
Data truncated due to size.\
""")

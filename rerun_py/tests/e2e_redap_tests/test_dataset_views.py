"""Tests for the DatasetView API."""

from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa
import pytest

if TYPE_CHECKING:
    from pathlib import Path

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


def sort_schema(schema: pa.Schema) -> pa.Schema:
    """Sort schema fields by name for order-independent comparison."""
    metadata = schema.metadata
    sorted_fields = sorted(schema, key=lambda field: field.name)
    return pa.schema(sorted_fields).with_metadata(metadata)


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

    assert str(df) == snapshot()

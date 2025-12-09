"""Tests for the DatasetView API."""

from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa
import rerun as rr

if TYPE_CHECKING:
    from pathlib import Path


def test_dataset_view_filter_segments(complex_dataset_prefix: Path, tmp_path: Path) -> None:
    """Test filtering a dataset by segment IDs."""
    with rr.server.Server() as server:
        client = server.client()
        ds = client.create_dataset("complex_dataset")
        ds.register_prefix(complex_dataset_prefix.as_uri())

        # Simple filter by segment list
        view = ds.filter_segments(["complex_recording_2"])
        assert sorted(view.segment_ids()) == ["complex_recording_2"]

        # Filter with metadata table
        meta = client.create_table(
            "metadata",
            pa.schema([
                ("rerun_segment_id", pa.string()),
                ("success", pa.bool_()),
            ]),
            tmp_path.as_uri(),
        )
        meta.append(
            rerun_segment_id=[
                "complex_recording_0",
                "complex_recording_1",
                "complex_recording_2",
                "complex_recording_3",
                "complex_recording_4",
            ],
            success=[True, True, False, True, False],
        )

        # Filter to only successful segments using DataFrame
        from datafusion import col

        good_segments = meta.df().filter(col("success"))
        good_view = ds.filter_segments(good_segments)

        assert sorted(good_view.segment_ids()) == [
            "complex_recording_0",
            "complex_recording_1",
            "complex_recording_3",
        ]


def test_dataset_view_filter_contents(complex_dataset_prefix: Path) -> None:
    """Test filtering a dataset by entity paths."""
    with rr.server.Server() as server:
        client = server.client()
        ds = client.create_dataset("complex_dataset")
        ds.register_prefix(complex_dataset_prefix.as_uri())

        # Get column names from original schema
        original_columns = sorted(f.name for f in ds.arrow_schema())
        assert "/points:Points2D:colors" in original_columns
        assert "/points:Points2D:positions" in original_columns
        assert "/text:TextLog:text" in original_columns

        # Filter to only /points entities
        view = ds.filter_contents(["/points/**"])
        filtered_columns = sorted(f.name for f in view.arrow_schema())

        assert "/points:Points2D:colors" in filtered_columns
        assert "/points:Points2D:positions" in filtered_columns
        assert "/text:TextLog:text" not in filtered_columns


def test_dataset_view_schema(complex_dataset_prefix: Path) -> None:
    """Test that schema reflects content filters."""
    with rr.server.Server() as server:
        client = server.client()
        ds = client.create_dataset("complex_dataset")
        ds.register_prefix(complex_dataset_prefix.as_uri())

        # Filter to /points entities
        view = ds.filter_contents(["/points/**"])
        schema = view.schema()

        # Check component columns are filtered
        component_paths = [col.entity_path for col in schema.component_columns()]
        assert "/points" in component_paths
        assert "/text" not in component_paths

        # Check column_names includes index and filtered components
        names = schema.column_names()
        assert "timeline" in names
        assert "/points:Points2D:colors" in names
        assert "/points:Points2D:positions" in names


def test_dataset_view_chained_filters(complex_dataset_prefix: Path) -> None:
    """Test chaining segment and content filters."""
    with rr.server.Server() as server:
        client = server.client()
        ds = client.create_dataset("complex_dataset")
        ds.register_prefix(complex_dataset_prefix.as_uri())

        # Chain filters
        view = ds.filter_segments(["complex_recording_0", "complex_recording_1"]).filter_contents(["/text/**"])

        # Check segment filter applied
        assert sorted(view.segment_ids()) == ["complex_recording_0", "complex_recording_1"]

        # Check content filter applied to schema
        column_names = [f.name for f in view.arrow_schema()]
        assert "/text:TextLog:text" in column_names
        assert "/points:Points2D:colors" not in column_names


def test_dataset_view_reader(complex_dataset_prefix: Path) -> None:
    """Test reading data through a DatasetView."""
    with rr.server.Server() as server:
        client = server.client()
        ds = client.create_dataset("complex_dataset")
        ds.register_prefix(complex_dataset_prefix.as_uri())

        # Create filtered view and read
        view = ds.filter_segments(["complex_recording_0"]).filter_contents(["/text/**"])
        df = view.reader(index="timeline")

        # Should have rows from the filtered segment with text data
        assert "rerun_segment_id" in df.schema().names
        assert "/text:TextLog:text" in df.schema().names


def test_dataset_view_get_index_ranges(complex_dataset_prefix: Path) -> None:
    """Test getting index ranges from a DatasetView."""
    with rr.server.Server() as server:
        client = server.client()
        ds = client.create_dataset("complex_dataset")
        ds.register_prefix(complex_dataset_prefix.as_uri())

        view = ds.filter_segments(["complex_recording_0"])
        ranges_df = view.get_index_ranges("timeline")

        # Should have columns for min/max
        column_names = ranges_df.schema().names
        assert "rerun_segment_id" in column_names
        assert "timeline:min" in column_names
        assert "timeline:max" in column_names

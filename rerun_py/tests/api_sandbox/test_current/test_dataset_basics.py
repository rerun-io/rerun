from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa
import pytest
import rerun as rr
from inline_snapshot import snapshot as inline_snapshot

if TYPE_CHECKING:
    from pathlib import Path


def test_dataset_basics(complex_dataset_prefix: Path) -> None:
    with rr.server.Server() as server:
        client = server.client()

        ds = client.create_dataset("basic_dataset")

        ds.register_prefix(complex_dataset_prefix.as_uri())

        partition_df = ds.segment_table()

        assert partition_df.schema().to_string(show_field_metadata=False) == inline_snapshot("""\
rerun_segment_id: string not null
rerun_layer_names: list<rerun_layer_names: string not null> not null
  child 0, rerun_layer_names: string not null
rerun_storage_urls: list<rerun_storage_urls: string not null> not null
  child 0, rerun_storage_urls: string not null
rerun_last_updated_at: timestamp[ns] not null
rerun_num_chunks: uint64 not null
rerun_size_bytes: uint64 not null
timeline:end: timestamp[ns]
timeline:start: timestamp[ns]
-- schema metadata --
sorbet:version: '0.1.2'\
""")

        assert str(
            partition_df.drop("rerun_storage_urls", "rerun_last_updated_at").sort("rerun_segment_id")
        ) == inline_snapshot("""\
┌─────────────────────┬───────────────────┬──────────────────┬──────────────────┬──────────────────────────────┬──────────────────────────────┐
│ rerun_segment_id    ┆ rerun_layer_names ┆ rerun_num_chunks ┆ rerun_size_bytes ┆ timeline:end                 ┆ timeline:start               │
│ ---                 ┆ ---               ┆ ---              ┆ ---              ┆ ---                          ┆ ---                          │
│ type: Utf8          ┆ type: List[Utf8]  ┆ type: u64        ┆ type: u64        ┆ type: nullable Timestamp(ns) ┆ type: nullable Timestamp(ns) │
│                     ┆                   ┆                  ┆                  ┆ index: timeline              ┆ index: timeline              │
│                     ┆                   ┆                  ┆                  ┆ index_kind: timestamp        ┆ index_kind: timestamp        │
│                     ┆                   ┆                  ┆                  ┆ index_marker: end            ┆ index_marker: start          │
│                     ┆                   ┆                  ┆                  ┆ kind: index                  ┆ kind: index                  │
╞═════════════════════╪═══════════════════╪══════════════════╪══════════════════╪══════════════════════════════╪══════════════════════════════╡
│ complex_recording_0 ┆ [base]            ┆ 3                ┆ 3326             ┆ 2000-01-01T00:00:02          ┆ 2000-01-01T00:00:00          │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ complex_recording_1 ┆ [base]            ┆ 3                ┆ 3326             ┆ 2000-01-01T00:00:03          ┆ 2000-01-01T00:00:01          │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ complex_recording_2 ┆ [base]            ┆ 3                ┆ 3326             ┆ 2000-01-01T00:00:04          ┆ 2000-01-01T00:00:02          │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ complex_recording_3 ┆ [base]            ┆ 3                ┆ 3326             ┆ 2000-01-01T00:00:05          ┆ 2000-01-01T00:00:03          │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ complex_recording_4 ┆ [base]            ┆ 3                ┆ 3326             ┆ 2000-01-01T00:00:06          ┆ 2000-01-01T00:00:04          │
└─────────────────────┴───────────────────┴──────────────────┴──────────────────┴──────────────────────────────┴──────────────────────────────┘\
""")


def test_dataset_schema(complex_dataset_prefix: Path) -> None:
    with rr.server.Server() as server:
        client = server.client()
        ds = client.create_dataset("complex_dataset")
        ds.register_prefix(complex_dataset_prefix.as_uri())

        assert str(ds.schema()) == inline_snapshot("""\
Index(timeline:timeline)
Column name: /points:Points2D:colors
	Entity path: /points
	Archetype: rerun.archetypes.Points2D
	Component type: rerun.components.Color
	Component: Points2D:colors
Column name: /points:Points2D:positions
	Entity path: /points
	Archetype: rerun.archetypes.Points2D
	Component type: rerun.components.Position2D
	Component: Points2D:positions
Column name: /text:TextLog:text
	Entity path: /text
	Archetype: rerun.archetypes.TextLog
	Component type: rerun.components.Text
	Component: TextLog:text
Column name: property:RecordingInfo:start_time
	Entity path: /__properties
	Archetype: rerun.archetypes.RecordingInfo
	Component type: rerun.components.Timestamp
	Component: RecordingInfo:start_time
	Static: true\
""")


def test_dataset_metadata(complex_dataset_prefix: Path, tmp_path: Path) -> None:
    with rr.server.Server() as server:
        client = server.client()

        ds = client.create_dataset("basic_dataset")
        ds.register_prefix(complex_dataset_prefix.as_uri())

        # TODO(jleibs): Consider attaching this metadata table directly to the dataset
        # and automatically joining it by default
        meta = client.create_table(
            "basic_dataset_metadata",
            pa.schema([
                ("rerun_segment_id", pa.string()),
                ("success", pa.bool_()),
            ]),
            tmp_path.as_uri(),
        )

        meta.append(
            rerun_segment_id=["complex_recording_0", "complex_recording_1", "complex_recording_4"],
            success=[True, False, True],
        )

        assert (str(meta.reader())) == inline_snapshot("""\
┌─────────────────────┬─────────────────────┐
│ rerun_segment_id    ┆ success             │
│ ---                 ┆ ---                 │
│ type: nullable Utf8 ┆ type: nullable bool │
╞═════════════════════╪═════════════════════╡
│ complex_recording_0 ┆ true                │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ complex_recording_1 ┆ false               │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ complex_recording_4 ┆ true                │
└─────────────────────┴─────────────────────┘\
""")


def test_schema_column_for_selector(complex_dataset_prefix: Path) -> None:
    """Test Schema.column_for_selector with various inputs and error cases."""
    with rr.server.Server() as server:
        client = server.client()
        ds = client.create_dataset("test_dataset")
        ds.register_prefix(complex_dataset_prefix.as_uri())

        schema = ds.schema()

        # Success case: valid selector string returns correct descriptor
        col = schema.column_for_selector("/points:Points2D:colors")
        assert col.entity_path == "/points"
        assert col.component == "Points2D:colors"

        # Success case: ComponentColumnSelector
        selector = rr.catalog.ComponentColumnSelector("/points", "Points2D:positions")
        col = schema.column_for_selector(selector)
        assert col.entity_path == "/points"
        assert col.component == "Points2D:positions"

        # Success case: ComponentColumnDescriptor passthrough (returns equivalent descriptor)
        existing_col = schema.column_for_selector("/text:TextLog:text")
        same_col = schema.column_for_selector(existing_col)
        assert same_col == existing_col

        # LookupError case: column not found
        with pytest.raises(LookupError):
            schema.column_for_selector("/nonexistent:Foo:bar")

        # ValueError case: invalid selector format (no colon)
        with pytest.raises(ValueError):
            schema.column_for_selector("invalid-format-no-colon")

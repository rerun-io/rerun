from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa
import pytest
import rerun_draft as rr
from inline_snapshot import snapshot as inline_snapshot

from .utils import sorted_schema_str

if TYPE_CHECKING:
    from collections.abc import Generator
    from pathlib import Path


@pytest.fixture
def rrd_paths(complex_dataset_prefix: Path) -> Generator[list[Path], None, None]:
    """Paths to some rrd files."""

    yield sorted(complex_dataset_prefix.glob("*.rrd"), key=lambda p: p.stem)


def test_dataset_basics(complex_dataset_prefix: Path) -> None:
    with rr.server.Server() as server:
        client = server.client()

        ds = client.create_dataset("basic_dataset")

        ds.register_prefix(complex_dataset_prefix.as_uri()).wait()

        segment_df = ds.segment_table()

        assert segment_df.schema().to_string(show_field_metadata=False) == inline_snapshot("""\
rerun_segment_id: string not null
rerun_layer_names: list<rerun_layer_names: string not null> not null
  child 0, rerun_layer_names: string not null
rerun_storage_urls: list<rerun_storage_urls: string not null> not null
  child 0, rerun_storage_urls: string not null
rerun_last_updated_at: timestamp[ns] not null
rerun_num_chunks: uint64 not null
rerun_size_bytes: uint64 not null
property:RecordingInfo:start_time: list<item: int64>
  child 0, item: int64
timeline:end: timestamp[ns]
timeline:start: timestamp[ns]
-- schema metadata --
sorbet:version: '0.1.3'\
""")

        assert str(
            segment_df.drop("rerun_storage_urls", "rerun_last_updated_at", "property:RecordingInfo:start_time").sort(
                "rerun_segment_id"
            )
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
│ complex_recording_0 ┆ [base]            ┆ 3                ┆ 4226             ┆ 2000-01-01T00:00:02          ┆ 2000-01-01T00:00:00          │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ complex_recording_1 ┆ [base]            ┆ 3                ┆ 4226             ┆ 2000-01-01T00:00:03          ┆ 2000-01-01T00:00:01          │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ complex_recording_2 ┆ [base]            ┆ 3                ┆ 4226             ┆ 2000-01-01T00:00:04          ┆ 2000-01-01T00:00:02          │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ complex_recording_3 ┆ [base]            ┆ 3                ┆ 4226             ┆ 2000-01-01T00:00:05          ┆ 2000-01-01T00:00:03          │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ complex_recording_4 ┆ [base]            ┆ 3                ┆ 4226             ┆ 2000-01-01T00:00:06          ┆ 2000-01-01T00:00:04          │
└─────────────────────┴───────────────────┴──────────────────┴──────────────────┴──────────────────────────────┴──────────────────────────────┘\
""")


def test_dataset_register(rrd_paths: list[Path]) -> None:
    with rr.server.Server() as server:
        client = server.client()

        ds = client.create_dataset("dataset")

        # Single RRD, default layer name
        ds.register(rrd_paths[0].as_uri()).wait()

        # Single RRD, override layer name
        ds.register(rrd_paths[1].as_uri(), layer_name="extra").wait()

        # Multiple RRDs, multiple layer names
        ds.register([p.as_uri() for p in rrd_paths[2:4]], layer_name=["fiz", "fuz"]).wait()

        # Multiple RRDs, single layer name
        ds.register([p.as_uri() for p in rrd_paths], layer_name="more").wait()

        with pytest.raises(ValueError):
            ds.register([p.as_uri() for p in rrd_paths], layer_name=["not", "enough"]).wait()

        assert str(
            ds.manifest().select("rerun_layer_name", "rerun_segment_id").sort("rerun_layer_name", "rerun_segment_id")
        ) == inline_snapshot(
            """\
┌────────────────────────────────────────────┐
│ METADATA:                                  │
│ * version: 0.1.3                           │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌──────────────────┬─────────────────────┐ │
│ │ rerun_layer_name ┆ rerun_segment_id    │ │
│ │ ---              ┆ ---                 │ │
│ │ type: Utf8       ┆ type: Utf8          │ │
│ ╞══════════════════╪═════════════════════╡ │
│ │ base             ┆ complex_recording_0 │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ extra            ┆ complex_recording_1 │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ fiz              ┆ complex_recording_2 │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ fuz              ┆ complex_recording_3 │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ more             ┆ complex_recording_0 │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ more             ┆ complex_recording_1 │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ more             ┆ complex_recording_2 │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ more             ┆ complex_recording_3 │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ more             ┆ complex_recording_4 │ │
│ └──────────────────┴─────────────────────┘ │
└────────────────────────────────────────────┘\
"""
        )


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

        assert sorted_schema_str(ds.arrow_schema(), with_metadata=True) == inline_snapshot("""\
/points:Points2D:colors: list<item: uint32>
  -- field metadata --
  rerun:archetype: 'rerun.archetypes.Points2D'
  rerun:component: 'Points2D:colors'
  rerun:component_type: 'rerun.components.Color'
  rerun:entity_path: '/points'
  rerun:kind: 'data'
/points:Points2D:positions: list<item: fixed_size_list<item: float not null>[2]>
  -- field metadata --
  rerun:archetype: 'rerun.archetypes.Points2D'
  rerun:component: 'Points2D:positions'
  rerun:component_type: 'rerun.components.Position2D'
  rerun:entity_path: '/points'
  rerun:kind: 'data'
/text:TextLog:text: list<item: string>
  -- field metadata --
  rerun:archetype: 'rerun.archetypes.TextLog'
  rerun:component: 'TextLog:text'
  rerun:component_type: 'rerun.components.Text'
  rerun:entity_path: '/text'
  rerun:kind: 'data'
property:RecordingInfo:start_time: list<item: int64>
  -- field metadata --
  rerun:archetype: 'rerun.archetypes.RecordingInfo'
  rerun:component: 'RecordingInfo:start_time'
  rerun:component_type: 'rerun.components.Timestamp'
  rerun:entity_path: '/__properties'
  rerun:is_static: 'true'
  rerun:kind: 'data'
rerun.controls.RowId: fixed_size_binary[16]
  -- field metadata --
  ARROW:extension:metadata: '{"namespace":"row"}'
  ARROW:extension:name: 'rerun.datatypes.TUID'
  rerun:kind: 'control'
timeline: timestamp[ns]
  -- field metadata --
  rerun:index_name: 'timeline'
  rerun:kind: 'index'
-- schema metadata --
sorbet:version: '0.1.3'\
""")


def test_dataset_metadata(complex_dataset_prefix: Path) -> None:
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


def test_manifest_diagnostic_data(complex_dataset_prefix: Path) -> None:
    """Test the include_diagnostic_data parameter on manifest()."""
    with rr.server.Server() as server:
        client = server.client()
        ds = client.create_dataset("dataset")
        ds.register_prefix(complex_dataset_prefix.as_uri()).wait()

        # Default: rerun_registration_status column should not be present
        manifest = ds.manifest()
        column_names = [f.name for f in manifest.schema()]
        assert "rerun_registration_status" not in column_names

        # With include_diagnostic_data=True: column should be present
        manifest_diag = ds.manifest(include_diagnostic_data=True)
        column_names_diag = [f.name for f in manifest_diag.schema()]
        assert "rerun_registration_status" in column_names_diag

        # In re_server, all registrations are successful (Done=1)
        # since schema conflicts fail synchronously
        statuses = manifest_diag.select("rerun_registration_status").to_arrow_table().to_pydict()
        assert all(s == "done" for s in statuses["rerun_registration_status"])

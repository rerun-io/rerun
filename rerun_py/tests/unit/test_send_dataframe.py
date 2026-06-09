"""Tests for rr.send_dataframe and rr.send_record_batch."""

from __future__ import annotations

import uuid
from typing import TYPE_CHECKING

import pyarrow as pa
import pytest
import rerun as rr
from inline_snapshot import snapshot as inline_snapshot
from rerun import (
    RERUN_KIND,
    RERUN_KIND_CONTROL,
    RERUN_KIND_INDEX,
    SORBET_ARCHETYPE_NAME,
    SORBET_COMPONENT,
    SORBET_COMPONENT_TYPE,
    SORBET_ENTITY_PATH,
    SORBET_INDEX_NAME,
)
from rerun.experimental import RrdReader

if TYPE_CHECKING:
    from collections.abc import Callable
    from pathlib import Path

    from rerun.experimental import Chunk
    from syrupy import SnapshotAssertion

APP_ID = "rerun_example_test_send_dataframe"


def _filter_rerun_columns(table: pa.Table) -> pa.Table:
    """Filter to only include columns with proper rerun metadata (skip rerun_segment_id)."""

    cols_to_keep = []
    for field in table.schema:
        if field.name == "log_time":
            # changes every run
            continue

        if field.metadata is None or b"rerun:kind" not in field.metadata:
            continue

        cols_to_keep.append(field.name)
    return table.select(cols_to_keep)


def test_send_dataframe_roundtrip(tmp_path: Path, snapshot: SnapshotAssertion) -> None:
    """Test that send_dataframe can roundtrip data through Server + Catalog API."""
    original_dir = tmp_path / "original"
    original_dir.mkdir()
    rrd_path = original_dir / "recording.rrd"

    # Create initial recording with some data
    with rr.RecordingStream(APP_ID, recording_id=uuid.uuid4()) as rec:
        rec.save(str(rrd_path))
        rec.set_time("my_index", sequence=1)
        rec.log("points", rr.Points3D([[1, 2, 3], [4, 5, 6], [7, 8, 9]], radii=[0.5]))
        rec.set_time("my_index", sequence=7)
        rec.log("points", rr.Points3D([[10, 11, 12]], colors=[[255, 0, 0]]))

    # Load via Server + Catalog API and read as Arrow table
    with rr.server.Server(datasets={"test_dataset": original_dir}) as server:
        ds = server.client().get_dataset("test_dataset")
        original_table = _filter_rerun_columns(ds.reader(index="my_index").to_arrow_table())

    # Send via send_dataframe to a new recording
    roundtrip_dir = tmp_path / "roundtrip"
    roundtrip_dir.mkdir()
    rrd2_path = roundtrip_dir / "recording.rrd"
    with rr.RecordingStream(APP_ID + "_roundtrip", recording_id=uuid.uuid4()) as rec2:
        rec2.save(str(rrd2_path))
        rr.send_dataframe(original_table, recording=rec2)

    # Verify roundtrip via catalog API - data should be identical
    with rr.server.Server(datasets={"roundtrip_dataset": roundtrip_dir}) as server:
        ds = server.client().get_dataset("roundtrip_dataset")
        roundtrip_table = _filter_rerun_columns(ds.reader(index="my_index").to_arrow_table())

    assert original_table == roundtrip_table
    assert str(original_table) == snapshot()


# A simple list-of-floats component column, two rows.
_VALUES = pa.array([[1.0], [2.0]], type=pa.list_(pa.float32()))


@pytest.fixture
def send_dataframe_and_get_chunks(tmp_path: Path) -> Callable[[pa.Table | pa.RecordBatchReader], list[Chunk]]:
    """Send a table/reader via `send_dataframe`, then read the result back as sorted chunks."""
    counter = 0

    def _impl(df: pa.Table | pa.RecordBatchReader) -> list[Chunk]:
        nonlocal counter
        counter += 1
        out_path = tmp_path / f"out_{counter}.rrd"
        with rr.RecordingStream(APP_ID, recording_id="characterization", send_properties=False) as rec:
            rec.save(out_path)
            rr.send_dataframe(df, recording=rec)
        chunks = RrdReader(out_path).stream().to_chunks()
        return sorted(chunks, key=lambda c: c.entity_path)

    return _impl


def _summary(chunks: list[Chunk]) -> list[str]:
    """One compact, redacted line per chunk — entity path, timelines, and components."""
    return [
        f"{c.entity_path} static={c.is_static} timelines={sorted(c.timeline_names)} ncols={c.num_columns}"
        for c in chunks
    ]


def test_full_metadata_single_entity(
    send_dataframe_and_get_chunks: Callable[[pa.Table | pa.RecordBatchReader], list[Chunk]],
) -> None:
    """Fully-tagged index + component column, mirroring the `send_dataframe` doc snippet."""
    index = pa.array([0, 1], type=pa.int64())
    schema = pa.schema([
        pa.field("frame", index.type, metadata={SORBET_INDEX_NAME: b"frame", RERUN_KIND: RERUN_KIND_INDEX}),
        pa.field(
            "/points:Points3D:positions",
            _VALUES.type,
            metadata={
                SORBET_ENTITY_PATH: b"/points",
                SORBET_ARCHETYPE_NAME: b"rerun.archetypes.Points3D",
                SORBET_COMPONENT: b"Points3D:positions",
                SORBET_COMPONENT_TYPE: b"rerun.components.Position3D",
                RERUN_KIND: b"data",
            },
        ),
    ])
    [chunk] = send_dataframe_and_get_chunks(pa.Table.from_arrays([index, _VALUES], schema=schema))
    assert chunk.format(redact=True) == inline_snapshot("""\
┌───────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                             │
│ * entity_path: /points                                                                                │
│ * id: [**REDACTED**]                                                                                  │
│ * version: [**REDACTED**]                                                                             │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌───────────────────────────────────────────────┬───────────────────┬───────────────────────────────┐ │
│ │ RowId                                         ┆ frame             ┆ Points3D:positions            │ │
│ │ ---                                           ┆ ---               ┆ ---                           │ │
│ │ type: non-null FixedSizeBinary(16)            ┆ type: Int64       ┆ type: List(Float32)           │ │
│ │ ARROW:extension:metadata: {"namespace":"row"} ┆ index_name: frame ┆ archetype: Points3D           │ │
│ │ ARROW:extension:name: TUID                    ┆ is_sorted: true   ┆ component: Points3D:positions │ │
│ │ is_sorted: true                               ┆ kind: index       ┆ component_type: Position3D    │ │
│ │ kind: control                                 ┆                   ┆ kind: data                    │ │
│ ╞═══════════════════════════════════════════════╪═══════════════════╪═══════════════════════════════╡ │
│ │ row_[**REDACTED**]                            ┆ 0                 ┆ [1.0]                         │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ row_[**REDACTED**]                            ┆ 1                 ┆ [2.0]                         │ │
│ └───────────────────────────────────────────────┴───────────────────┴───────────────────────────────┘ │
└───────────────────────────────────────────────────────────────────────────────────────────────────────┘\
""")


def test_index_kind_without_index_name(
    send_dataframe_and_get_chunks: Callable[[pa.Table | pa.RecordBatchReader], list[Chunk]],
) -> None:
    """A `kind=index` column with no `index_name` becomes a timeline named after the column."""
    index = pa.array([0, 1], type=pa.int64())
    schema = pa.schema([
        pa.field("my_time", index.type, metadata={RERUN_KIND: RERUN_KIND_INDEX}),
        pa.field(
            "/e:C:c", _VALUES.type, metadata={SORBET_ENTITY_PATH: b"/e", SORBET_COMPONENT: b"C:c", RERUN_KIND: b"data"}
        ),
    ])
    [chunk] = send_dataframe_and_get_chunks(pa.Table.from_arrays([index, _VALUES], schema=schema))
    assert chunk.timeline_names == inline_snapshot(["my_time"])


def test_entity_path_from_column_name(
    send_dataframe_and_get_chunks: Callable[[pa.Table | pa.RecordBatchReader], list[Chunk]],
) -> None:
    """With no `entity_path` metadata, the entity path is the column name up to the first ':'."""
    index = pa.array([0, 1], type=pa.int64())
    schema = pa.schema([
        pa.field("frame", index.type, metadata={SORBET_INDEX_NAME: b"frame", RERUN_KIND: RERUN_KIND_INDEX}),
        pa.field("/points:Points3D:positions", _VALUES.type, metadata={SORBET_COMPONENT: b"Points3D:positions"}),
    ])
    [chunk] = send_dataframe_and_get_chunks(pa.Table.from_arrays([index, _VALUES], schema=schema))
    assert chunk.entity_path == inline_snapshot("/points")


def test_multiple_entities(
    send_dataframe_and_get_chunks: Callable[[pa.Table | pa.RecordBatchReader], list[Chunk]],
) -> None:
    """Component columns with different entity paths split into one chunk per entity."""
    index = pa.array([0, 1], type=pa.int64())
    schema = pa.schema([
        pa.field("frame", index.type, metadata={SORBET_INDEX_NAME: b"frame", RERUN_KIND: RERUN_KIND_INDEX}),
        pa.field(
            "/a:C:c", _VALUES.type, metadata={SORBET_ENTITY_PATH: b"/a", SORBET_COMPONENT: b"C:c", RERUN_KIND: b"data"}
        ),
        pa.field(
            "/b:C:c", _VALUES.type, metadata={SORBET_ENTITY_PATH: b"/b", SORBET_COMPONENT: b"C:c", RERUN_KIND: b"data"}
        ),
    ])
    chunks = send_dataframe_and_get_chunks(pa.Table.from_arrays([index, _VALUES, _VALUES], schema=schema))
    assert _summary(chunks) == inline_snapshot([
        "/a static=False timelines=['frame'] ncols=3",
        "/b static=False timelines=['frame'] ncols=3",
    ])


def test_control_column_skipped(
    send_dataframe_and_get_chunks: Callable[[pa.Table | pa.RecordBatchReader], list[Chunk]],
) -> None:
    """A `kind=control` column is dropped — it becomes neither a timeline nor a component."""
    index = pa.array([0, 1], type=pa.int64())
    control = pa.array([10, 20], type=pa.int64())
    schema = pa.schema([
        pa.field("frame", index.type, metadata={SORBET_INDEX_NAME: b"frame", RERUN_KIND: RERUN_KIND_INDEX}),
        pa.field("ctrl", control.type, metadata={RERUN_KIND: RERUN_KIND_CONTROL}),
        pa.field(
            "/e:C:c", _VALUES.type, metadata={SORBET_ENTITY_PATH: b"/e", SORBET_COMPONENT: b"C:c", RERUN_KIND: b"data"}
        ),
    ])
    [chunk] = send_dataframe_and_get_chunks(pa.Table.from_arrays([index, control, _VALUES], schema=schema))
    # `ctrl` must not appear as a column; only RowId + frame + the component remain.
    assert chunk.format(redact=True) == inline_snapshot("""\
┌─────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                       │
│ * entity_path: /e                                                                               │
│ * id: [**REDACTED**]                                                                            │
│ * version: [**REDACTED**]                                                                       │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌───────────────────────────────────────────────┬───────────────────┬─────────────────────────┐ │
│ │ RowId                                         ┆ frame             ┆ C:c                     │ │
│ │ ---                                           ┆ ---               ┆ ---                     │ │
│ │ type: non-null FixedSizeBinary(16)            ┆ type: Int64       ┆ type: List(Float32)     │ │
│ │ ARROW:extension:metadata: {"namespace":"row"} ┆ index_name: frame ┆ component: C:c          │ │
│ │ ARROW:extension:name: TUID                    ┆ is_sorted: true   ┆ component_type: Unknown │ │
│ │ is_sorted: true                               ┆ kind: index       ┆ kind: data              │ │
│ │ kind: control                                 ┆                   ┆                         │ │
│ ╞═══════════════════════════════════════════════╪═══════════════════╪═════════════════════════╡ │
│ │ row_[**REDACTED**]                            ┆ 0                 ┆ [1.0]                   │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ row_[**REDACTED**]                            ┆ 1                 ┆ [2.0]                   │ │
│ └───────────────────────────────────────────────┴───────────────────┴─────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────────────────────────┘\
""")


def test_component_type_defaults_to_unknown(
    send_dataframe_and_get_chunks: Callable[[pa.Table | pa.RecordBatchReader], list[Chunk]],
) -> None:
    """A component column with no `component_type` metadata defaults to `Unknown`."""
    index = pa.array([0, 1], type=pa.int64())
    schema = pa.schema([
        pa.field("frame", index.type, metadata={SORBET_INDEX_NAME: b"frame", RERUN_KIND: RERUN_KIND_INDEX}),
        pa.field(
            "/e:thing",
            _VALUES.type,
            metadata={SORBET_ENTITY_PATH: b"/e", SORBET_COMPONENT: b"thing", RERUN_KIND: b"data"},
        ),
    ])
    [chunk] = send_dataframe_and_get_chunks(pa.Table.from_arrays([index, _VALUES], schema=schema))
    assert chunk.format(redact=True, trim_metadata_keys=False) == inline_snapshot("""\
┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                                   │
│ * rerun:entity_path: /e                                                                                     │
│ * rerun:id: [**REDACTED**]                                                                                  │
│ * sorbet:version: [**REDACTED**]                                                                            │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌───────────────────────────────────────────────┬─────────────────────────┬───────────────────────────────┐ │
│ │ RowId                                         ┆ frame                   ┆ thing                         │ │
│ │ ---                                           ┆ ---                     ┆ ---                           │ │
│ │ type: non-null FixedSizeBinary(16)            ┆ type: Int64             ┆ type: List(Float32)           │ │
│ │ ARROW:extension:metadata: {"namespace":"row"} ┆ rerun:index_name: frame ┆ rerun:component: thing        │ │
│ │ ARROW:extension:name: TUID                    ┆ rerun:is_sorted: true   ┆ rerun:component_type: Unknown │ │
│ │ rerun:is_sorted: true                         ┆ rerun:kind: index       ┆ rerun:kind: data              │ │
│ │ rerun:kind: control                           ┆                         ┆                               │ │
│ ╞═══════════════════════════════════════════════╪═════════════════════════╪═══════════════════════════════╡ │
│ │ row_[**REDACTED**]                            ┆ 0                       ┆ [1.0]                         │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ row_[**REDACTED**]                            ┆ 1                       ┆ [2.0]                         │ │
│ └───────────────────────────────────────────────┴─────────────────────────┴───────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────────┘\
""")


def test_static_no_index(
    send_dataframe_and_get_chunks: Callable[[pa.Table | pa.RecordBatchReader], list[Chunk]],
) -> None:
    """With no index column, the resulting chunk is static."""
    schema = pa.schema([
        pa.field(
            "/e:C:c", _VALUES.type, metadata={SORBET_ENTITY_PATH: b"/e", SORBET_COMPONENT: b"C:c", RERUN_KIND: b"data"}
        ),
    ])
    [chunk] = send_dataframe_and_get_chunks(pa.Table.from_arrays([_VALUES], schema=schema))
    assert chunk.is_static == inline_snapshot(True)
    assert chunk.timeline_names == inline_snapshot([])


def test_record_batch_reader_input(
    send_dataframe_and_get_chunks: Callable[[pa.Table | pa.RecordBatchReader], list[Chunk]],
) -> None:
    """A `RecordBatchReader` produces the same result as the equivalent `Table`."""
    index = pa.array([0, 1], type=pa.int64())
    schema = pa.schema([
        pa.field("frame", index.type, metadata={SORBET_INDEX_NAME: b"frame", RERUN_KIND: RERUN_KIND_INDEX}),
        pa.field(
            "/e:C:c", _VALUES.type, metadata={SORBET_ENTITY_PATH: b"/e", SORBET_COMPONENT: b"C:c", RERUN_KIND: b"data"}
        ),
    ])
    table = pa.Table.from_arrays([index, _VALUES], schema=schema)
    [chunk] = send_dataframe_and_get_chunks(table.to_reader())
    assert _summary([chunk]) == inline_snapshot(["/e static=False timelines=['frame'] ncols=3"])

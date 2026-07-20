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
def send_dataframe_and_get_chunks(tmp_path: Path) -> Callable[..., list[Chunk]]:
    """Send a table/reader via `send_dataframe`, then read the result back as sorted chunks."""
    counter = 0

    def _impl(df: pa.Table | pa.RecordBatchReader, **kwargs: object) -> list[Chunk]:
        nonlocal counter
        counter += 1
        out_path = tmp_path / f"out_{counter}.rrd"
        with rr.RecordingStream(APP_ID, recording_id="characterization", send_properties=False) as rec:
            rec.save(out_path)
            rr.send_dataframe(df, recording=rec, **kwargs)  # type: ignore[arg-type]
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
    """A leading-`/` column name is split into entity path + component; otherwise it lands on root."""
    index = pa.array([0, 1], type=pa.int64())
    schema = pa.schema([
        pa.field("frame", index.type, metadata={SORBET_INDEX_NAME: b"frame", RERUN_KIND: RERUN_KIND_INDEX}),
        pa.field("/points:Points3D:positions", _VALUES.type, metadata={SORBET_COMPONENT: b"Points3D:positions"}),
    ])
    [chunk] = send_dataframe_and_get_chunks(pa.Table.from_arrays([index, _VALUES], schema=schema))
    assert chunk.entity_path == inline_snapshot("/points")


def test_entity_path_non_leading_slash_is_root(
    send_dataframe_and_get_chunks: Callable[[pa.Table | pa.RecordBatchReader], list[Chunk]],
) -> None:
    """A column name without a leading `/` is no longer parsed for an entity path; it lands on root."""
    index = pa.array([0, 1], type=pa.int64())
    schema = pa.schema([
        pa.field("frame", index.type, metadata={SORBET_INDEX_NAME: b"frame", RERUN_KIND: RERUN_KIND_INDEX}),
        pa.field("foo:bar", _VALUES.type, metadata={}),
    ])
    [chunk] = send_dataframe_and_get_chunks(pa.Table.from_arrays([index, _VALUES], schema=schema))
    assert chunk.entity_path == inline_snapshot("/")


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


def test_control_kind_is_treated_as_row_id() -> None:
    """
    A `kind=control` column is interpreted as a row-id column, not a component.

    Without a chunk id the batch is only *partially* identified, so it takes the mint path: the
    control column is dropped (rather than carried as a component) and fresh row ids are minted.
    """
    from rerun.experimental import Chunk

    index = pa.array([0, 1], type=pa.int64())
    control = pa.array([10, 20], type=pa.int64())
    schema = pa.schema([
        pa.field("frame", index.type, metadata={SORBET_INDEX_NAME: b"frame", RERUN_KIND: RERUN_KIND_INDEX}),
        pa.field("ctrl", control.type, metadata={RERUN_KIND: RERUN_KIND_CONTROL}),
        pa.field(
            "/e:C:c", _VALUES.type, metadata={SORBET_ENTITY_PATH: b"/e", SORBET_COMPONENT: b"C:c", RERUN_KIND: b"data"}
        ),
    ])
    rb = pa.RecordBatch.from_arrays([index, control, _VALUES], schema=schema)
    [chunk] = Chunk.from_record_batch(rb)
    formatted = chunk.format(redact=True, trim_metadata_keys=False)
    # The control column was consumed as a row-id and dropped, not carried as a component, and the
    # chunk carries a freshly-minted `RowId` column instead.
    assert "ctrl" not in formatted
    assert "rerun:component: C:c" in formatted
    assert "RowId" in formatted


def test_no_component_type_is_left_unset(
    send_dataframe_and_get_chunks: Callable[[pa.Table | pa.RecordBatchReader], list[Chunk]],
) -> None:
    """A component column with no `component_type` metadata leaves it unset (no `Unknown` default)."""
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
    formatted = chunk.format(redact=True, trim_metadata_keys=False)
    assert "rerun:component: thing" in formatted
    assert "rerun:component_type" not in formatted


def test_no_index_is_ambiguous() -> None:
    """With `index` left at the default (AUTO) and no index metadata, the batch is rejected."""
    from rerun.experimental import Chunk

    schema = pa.schema([
        pa.field(
            "/e:C:c", _VALUES.type, metadata={SORBET_ENTITY_PATH: b"/e", SORBET_COMPONENT: b"C:c", RERUN_KIND: b"data"}
        ),
    ])
    with pytest.raises(ValueError):
        Chunk.from_record_batch(pa.RecordBatch.from_arrays([_VALUES], schema=schema))


def test_static_index_none(
    send_dataframe_and_get_chunks: Callable[..., list[Chunk]],
) -> None:
    """`index=None` produces a static chunk."""
    one_value = pa.array([[1.0]], type=pa.list_(pa.float32()))
    schema = pa.schema([
        pa.field(
            "/e:C:c",
            one_value.type,
            metadata={SORBET_ENTITY_PATH: b"/e", SORBET_COMPONENT: b"C:c", RERUN_KIND: b"data"},
        ),
    ])
    table = pa.Table.from_arrays([one_value], schema=schema)
    [chunk] = send_dataframe_and_get_chunks(table, index=None)
    assert chunk.is_static == inline_snapshot(True)
    assert chunk.timeline_names == inline_snapshot([])


def test_static_index_none_with_index_metadata_is_contradiction(
    send_dataframe_and_get_chunks: Callable[..., list[Chunk]],
) -> None:
    """`index=None` plus index metadata in the batch is a contradiction and is rejected."""
    index = pa.array([0, 1], type=pa.int64())
    schema = pa.schema([
        pa.field("frame", index.type, metadata={SORBET_INDEX_NAME: b"frame", RERUN_KIND: RERUN_KIND_INDEX}),
        pa.field(
            "/e:C:c", _VALUES.type, metadata={SORBET_ENTITY_PATH: b"/e", SORBET_COMPONENT: b"C:c", RERUN_KIND: b"data"}
        ),
    ])
    with pytest.raises(ValueError):
        send_dataframe_and_get_chunks(pa.Table.from_arrays([index, _VALUES], schema=schema), index=None)


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

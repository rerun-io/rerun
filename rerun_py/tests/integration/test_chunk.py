"""Tests for Chunk construction from PyArrow data."""

from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa
import pytest
import rerun as rr
from inline_snapshot import snapshot as inline_snapshot
from rerun.experimental import Chunk, Lens, LensOutput, RrdReader, Selector

if TYPE_CHECKING:
    from pathlib import Path


# ---------------------------------------------------------------------------
# from_record_batch
# ---------------------------------------------------------------------------


def test_chunk_from_record_batch_round_trip(test_rrd_path: Path) -> None:
    """to_record_batch() -> from_record_batch() round-trips correctly."""
    chunks = RrdReader(test_rrd_path).stream().to_chunks()
    assert len(chunks) > 0

    for original in chunks:
        rb = original.to_record_batch()
        restored = Chunk.from_record_batch(rb)
        assert restored.entity_path == original.entity_path
        assert restored.num_rows == original.num_rows
        assert restored.num_columns == original.num_columns
        assert restored.is_static == original.is_static
        assert sorted(restored.timeline_names) == sorted(original.timeline_names)


def test_chunk_from_record_batch_rejects_plain_batch() -> None:
    """from_record_batch() raises on a RecordBatch without Rerun metadata."""

    plain_batch = pa.record_batch({"x": [1, 2, 3]})
    with pytest.raises(ValueError):
        Chunk.from_record_batch(plain_batch)


# ---------------------------------------------------------------------------
# from_columns
# ---------------------------------------------------------------------------


def test_chunk_from_columns_temporal() -> None:
    """from_columns() creates a temporal chunk mirroring send_columns API."""
    chunk = Chunk.from_columns(
        "/test/entity",
        indexes=[rr.TimeColumn("frame", sequence=[0, 1])],
        columns=rr.Points3D.columns(positions=[[1, 2, 3], [10, 20, 30], [4, 5, 6]]).partition(lengths=[2, 1]),
    )
    assert chunk.format(redact=True) == inline_snapshot("""\
┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                                               │
│ * entity_path: /test/entity                                                                                             │
│ * id: [**REDACTED**]                                                                                                    │
│ * version: [**REDACTED**]                                                                                               │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌───────────────────────────────────────────────┬───────────────────┬─────────────────────────────────────────────────┐ │
│ │ RowId                                         ┆ frame             ┆ Points3D:positions                              │ │
│ │ ---                                           ┆ ---               ┆ ---                                             │ │
│ │ type: non-null FixedSizeBinary(16)            ┆ type: Int64       ┆ type: List(FixedSizeList(3 x non-null Float32)) │ │
│ │ ARROW:extension:metadata: {"namespace":"row"} ┆ index_name: frame ┆ archetype: Points3D                             │ │
│ │ ARROW:extension:name: TUID                    ┆ is_sorted: true   ┆ component: Points3D:positions                   │ │
│ │ is_sorted: true                               ┆ kind: index       ┆ component_type: Position3D                      │ │
│ │ kind: control                                 ┆                   ┆ kind: data                                      │ │
│ ╞═══════════════════════════════════════════════╪═══════════════════╪═════════════════════════════════════════════════╡ │
│ │ row_[**REDACTED**]                            ┆ 0                 ┆ [[1.0, 2.0, 3.0], [10.0, 20.0, 30.0]]           │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ row_[**REDACTED**]                            ┆ 1                 ┆ [[4.0, 5.0, 6.0]]                               │ │
│ └───────────────────────────────────────────────┴───────────────────┴─────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘\
""")


def test_chunk_from_columns_static() -> None:
    """from_columns() with empty indexes creates a static chunk."""
    chunk = Chunk.from_columns(
        "/test/static",
        indexes=[],
        columns=rr.Points3D.columns(positions=[[1, 2, 3], [4, 5, 6]]),
    )
    assert chunk.format(redact=True) == inline_snapshot("""\
┌─────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                           │
│ * entity_path: /test/static                                                                         │
│ * id: [**REDACTED**]                                                                                │
│ * version: [**REDACTED**]                                                                           │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌───────────────────────────────────────────────┬─────────────────────────────────────────────────┐ │
│ │ RowId                                         ┆ Points3D:positions                              │ │
│ │ ---                                           ┆ ---                                             │ │
│ │ type: non-null FixedSizeBinary(16)            ┆ type: List(FixedSizeList(3 x non-null Float32)) │ │
│ │ ARROW:extension:metadata: {"namespace":"row"} ┆ archetype: Points3D                             │ │
│ │ ARROW:extension:name: TUID                    ┆ component: Points3D:positions                   │ │
│ │ is_sorted: true                               ┆ component_type: Position3D                      │ │
│ │ kind: control                                 ┆ kind: data                                      │ │
│ ╞═══════════════════════════════════════════════╪═════════════════════════════════════════════════╡ │
│ │ row_[**REDACTED**]                            ┆ [[1.0, 2.0, 3.0]]                               │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ row_[**REDACTED**]                            ┆ [[4.0, 5.0, 6.0]]                               │ │
│ └───────────────────────────────────────────────┴─────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────────────────────────────┘\
""")


def test_chunk_from_columns_into_store() -> None:
    """Chunks built via from_columns can be inserted into a ChunkStore."""
    from rerun.experimental import ChunkStore

    chunk = Chunk.from_columns(
        "/test",
        indexes=[rr.TimeColumn("frame", sequence=[0])],
        columns=rr.Points3D.columns(positions=[[1, 2, 3]]),
    )

    store = ChunkStore.from_chunks([chunk])
    assert len(store) == 1


def test_chunk_from_columns_multiple_timelines() -> None:
    """from_columns() with multiple timelines."""
    chunk = Chunk.from_columns(
        "/test/multi",
        indexes=[
            rr.TimeColumn("frame", sequence=[0, 1]),
            rr.TimeColumn("timestamp", timestamp=[1000.0, 2000.0]),
        ],
        columns=rr.Points3D.columns(positions=[[1, 2, 3], [4, 5, 6]]),
    )
    assert chunk.format(redact=True) == inline_snapshot("""\
┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                                                                       │
│ * entity_path: /test/multi                                                                                                                      │
│ * id: [**REDACTED**]                                                                                                                            │
│ * version: [**REDACTED**]                                                                                                                       │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌───────────────────────────────────────────────┬───────────────────┬───────────────────────┬─────────────────────────────────────────────────┐ │
│ │ RowId                                         ┆ frame             ┆ timestamp             ┆ Points3D:positions                              │ │
│ │ ---                                           ┆ ---               ┆ ---                   ┆ ---                                             │ │
│ │ type: non-null FixedSizeBinary(16)            ┆ type: Int64       ┆ type: Timestamp(ns)   ┆ type: List(FixedSizeList(3 x non-null Float32)) │ │
│ │ ARROW:extension:metadata: {"namespace":"row"} ┆ index_name: frame ┆ index_name: timestamp ┆ archetype: Points3D                             │ │
│ │ ARROW:extension:name: TUID                    ┆ is_sorted: true   ┆ is_sorted: true       ┆ component: Points3D:positions                   │ │
│ │ is_sorted: true                               ┆ kind: index       ┆ kind: index           ┆ component_type: Position3D                      │ │
│ │ kind: control                                 ┆                   ┆                       ┆ kind: data                                      │ │
│ ╞═══════════════════════════════════════════════╪═══════════════════╪═══════════════════════╪═════════════════════════════════════════════════╡ │
│ │ row_[**REDACTED**]                            ┆ 0                 ┆ 1970-01-01T00:16:40   ┆ [[1.0, 2.0, 3.0]]                               │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ row_[**REDACTED**]                            ┆ 1                 ┆ 1970-01-01T00:33:20   ┆ [[4.0, 5.0, 6.0]]                               │ │
│ └───────────────────────────────────────────────┴───────────────────┴───────────────────────┴─────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘\
""")


def test_chunk_from_columns_length_mismatch() -> None:
    """from_columns() raises ValueError when column lengths don't match."""
    with pytest.raises(ValueError, match="same length"):
        Chunk.from_columns(
            "/test/bad",
            indexes=[rr.TimeColumn("frame", sequence=[0, 1, 2])],  # 3 rows
            columns=rr.Points3D.columns(positions=[[1, 2, 3], [4, 5, 6]]),  # 2 rows
        )


# ---------------------------------------------------------------------------
# apply_lenses
# ---------------------------------------------------------------------------


def test_apply_lenses_field_extraction() -> None:
    """apply_lenses extracts a struct field as a new Scalar component."""
    imu_data = pa.StructArray.from_arrays(
        [pa.array([1.0, 2.0], type=pa.float64()), pa.array([3.0, 4.0], type=pa.float64())],
        names=["x", "y"],
    )
    chunk = Chunk.from_columns(
        "/sensor",
        indexes=[rr.TimeColumn("frame", sequence=[0, 1])],
        columns=rr.DynamicArchetype.columns(archetype="Imu", components={"accel": imu_data}),
    )

    assert chunk.format(redact=True) == inline_snapshot("""\
┌────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                                              │
│ * entity_path: /sensor                                                                                                 │
│ * id: [**REDACTED**]                                                                                                   │
│ * version: [**REDACTED**]                                                                                              │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌───────────────────────────────────────────────┬───────────────────┬────────────────────────────────────────────────┐ │
│ │ RowId                                         ┆ frame             ┆ Imu:accel                                      │ │
│ │ ---                                           ┆ ---               ┆ ---                                            │ │
│ │ type: non-null FixedSizeBinary(16)            ┆ type: Int64       ┆ type: List(Struct("x": Float64, "y": Float64)) │ │
│ │ ARROW:extension:metadata: {"namespace":"row"} ┆ index_name: frame ┆ archetype: Imu                                 │ │
│ │ ARROW:extension:name: TUID                    ┆ is_sorted: true   ┆ component: Imu:accel                           │ │
│ │ is_sorted: true                               ┆ kind: index       ┆ kind: data                                     │ │
│ │ kind: control                                 ┆                   ┆                                                │ │
│ ╞═══════════════════════════════════════════════╪═══════════════════╪════════════════════════════════════════════════╡ │
│ │ row_[**REDACTED**]                            ┆ 0                 ┆ [{x: 1.0, y: 3.0}]                             │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ row_[**REDACTED**]                            ┆ 1                 ┆ [{x: 2.0, y: 4.0}]                             │ │
│ └───────────────────────────────────────────────┴───────────────────┴────────────────────────────────────────────────┘ │
└────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘\
""")

    lens = Lens(
        "Imu:accel",
        LensOutput().to_component(rr.Scalars.descriptor_scalars(), ".x"),
    )
    results = chunk.apply_lenses(lens)

    assert len(results) == 1
    assert chunk.id != results[0].id
    assert results[0].format(redact=True) == inline_snapshot("""\
┌────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                          │
│ * entity_path: /sensor                                                                             │
│ * id: [**REDACTED**]                                                                               │
│ * version: [**REDACTED**]                                                                          │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌───────────────────────────────────────────────┬───────────────────┬────────────────────────────┐ │
│ │ RowId                                         ┆ frame             ┆ Scalars:scalars            │ │
│ │ ---                                           ┆ ---               ┆ ---                        │ │
│ │ type: non-null FixedSizeBinary(16)            ┆ type: Int64       ┆ type: List(Float64)        │ │
│ │ ARROW:extension:metadata: {"namespace":"row"} ┆ index_name: frame ┆ archetype: Scalars         │ │
│ │ ARROW:extension:name: TUID                    ┆ is_sorted: true   ┆ component: Scalars:scalars │ │
│ │ is_sorted: true                               ┆ kind: index       ┆ component_type: Scalar     │ │
│ │ kind: control                                 ┆                   ┆ kind: data                 │ │
│ ╞═══════════════════════════════════════════════╪═══════════════════╪════════════════════════════╡ │
│ │ row_[**REDACTED**]                            ┆ 0                 ┆ [1.0]                      │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ row_[**REDACTED**]                            ┆ 1                 ┆ [2.0]                      │ │
│ └───────────────────────────────────────────────┴───────────────────┴────────────────────────────┘ │
└────────────────────────────────────────────────────────────────────────────────────────────────────┘\
""")


def test_apply_lenses_no_match() -> None:
    """apply_lenses forwards the original chunk when no lens input component matches."""
    chunk = Chunk.from_columns(
        "/test",
        indexes=[rr.TimeColumn("frame", sequence=[0])],
        columns=rr.Points3D.columns(positions=[[1, 2, 3]]),
    )

    lens = Lens(
        "Nonexistent:foo",
        LensOutput().to_component("out:bar", "."),
    )
    results = chunk.apply_lenses(lens)
    assert len(results) == 1
    assert str(results[0]) == str(chunk)  # TODO(ab): we should have Chunk.__eq__


def test_apply_lenses_empty_list() -> None:
    """apply_lenses([]) forwards the original chunk unchanged."""
    chunk = Chunk.from_columns(
        "/test",
        indexes=[rr.TimeColumn("frame", sequence=[0])],
        columns=rr.Points3D.columns(positions=[[1, 2, 3]]),
    )
    results = chunk.apply_lenses([])
    assert len(results) == 1
    assert str(results[0]) == str(chunk)  # TODO(ab): we should have Chunk.__eq__


def test_apply_lenses_multiple_outputs() -> None:
    """A lens with multiple LensOutputs targeting different entities."""
    data = pa.StructArray.from_arrays(
        [pa.array([1.0]), pa.array([2.0])],
        names=["x", "y"],
    )
    chunk = Chunk.from_columns(
        "/sensor",
        indexes=[rr.TimeColumn("frame", sequence=[0])],
        columns=rr.DynamicArchetype.columns(archetype="Imu", components={"accel": data}),
    )

    assert chunk.format(redact=True) == inline_snapshot("""\
┌────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                                              │
│ * entity_path: /sensor                                                                                                 │
│ * id: [**REDACTED**]                                                                                                   │
│ * version: [**REDACTED**]                                                                                              │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌───────────────────────────────────────────────┬───────────────────┬────────────────────────────────────────────────┐ │
│ │ RowId                                         ┆ frame             ┆ Imu:accel                                      │ │
│ │ ---                                           ┆ ---               ┆ ---                                            │ │
│ │ type: non-null FixedSizeBinary(16)            ┆ type: Int64       ┆ type: List(Struct("x": Float64, "y": Float64)) │ │
│ │ ARROW:extension:metadata: {"namespace":"row"} ┆ index_name: frame ┆ archetype: Imu                                 │ │
│ │ ARROW:extension:name: TUID                    ┆ is_sorted: true   ┆ component: Imu:accel                           │ │
│ │ is_sorted: true                               ┆ kind: index       ┆ kind: data                                     │ │
│ │ kind: control                                 ┆                   ┆                                                │ │
│ ╞═══════════════════════════════════════════════╪═══════════════════╪════════════════════════════════════════════════╡ │
│ │ row_[**REDACTED**]                            ┆ 0                 ┆ [{x: 1.0, y: 2.0}]                             │ │
│ └───────────────────────────────────────────────┴───────────────────┴────────────────────────────────────────────────┘ │
└────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘\
""")

    lens = Lens(
        "Imu:accel",
        to_entity={
            "/out/x": LensOutput().to_component(rr.Scalars.descriptor_scalars(), ".x"),
            "/out/y": LensOutput().to_component(rr.Scalars.descriptor_scalars(), ".y"),
        },
    )
    results = chunk.apply_lenses(lens)

    assert len(results) == 2

    # The original chunk is not be forwarded as is, so it's id must not be visible here
    assert chunk.id not in {r.id for r in results}
    assert [r.format(redact=True) for r in results] == inline_snapshot([
        """\
┌────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                          │
│ * entity_path: /out/x                                                                              │
│ * id: [**REDACTED**]                                                                               │
│ * version: [**REDACTED**]                                                                          │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌───────────────────────────────────────────────┬───────────────────┬────────────────────────────┐ │
│ │ RowId                                         ┆ frame             ┆ Scalars:scalars            │ │
│ │ ---                                           ┆ ---               ┆ ---                        │ │
│ │ type: non-null FixedSizeBinary(16)            ┆ type: Int64       ┆ type: List(Float64)        │ │
│ │ ARROW:extension:metadata: {"namespace":"row"} ┆ index_name: frame ┆ archetype: Scalars         │ │
│ │ ARROW:extension:name: TUID                    ┆ is_sorted: true   ┆ component: Scalars:scalars │ │
│ │ is_sorted: true                               ┆ kind: index       ┆ component_type: Scalar     │ │
│ │ kind: control                                 ┆                   ┆ kind: data                 │ │
│ ╞═══════════════════════════════════════════════╪═══════════════════╪════════════════════════════╡ │
│ │ row_[**REDACTED**]                            ┆ 0                 ┆ [1.0]                      │ │
│ └───────────────────────────────────────────────┴───────────────────┴────────────────────────────┘ │
└────────────────────────────────────────────────────────────────────────────────────────────────────┘\
""",
        """\
┌────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                          │
│ * entity_path: /out/y                                                                              │
│ * id: [**REDACTED**]                                                                               │
│ * version: [**REDACTED**]                                                                          │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌───────────────────────────────────────────────┬───────────────────┬────────────────────────────┐ │
│ │ RowId                                         ┆ frame             ┆ Scalars:scalars            │ │
│ │ ---                                           ┆ ---               ┆ ---                        │ │
│ │ type: non-null FixedSizeBinary(16)            ┆ type: Int64       ┆ type: List(Float64)        │ │
│ │ ARROW:extension:metadata: {"namespace":"row"} ┆ index_name: frame ┆ archetype: Scalars         │ │
│ │ ARROW:extension:name: TUID                    ┆ is_sorted: true   ┆ component: Scalars:scalars │ │
│ │ is_sorted: true                               ┆ kind: index       ┆ component_type: Scalar     │ │
│ │ kind: control                                 ┆                   ┆ kind: data                 │ │
│ ╞═══════════════════════════════════════════════╪═══════════════════╪════════════════════════════╡ │
│ │ row_[**REDACTED**]                            ┆ 0                 ┆ [2.0]                      │ │
│ └───────────────────────────────────────────────┴───────────────────┴────────────────────────────┘ │
└────────────────────────────────────────────────────────────────────────────────────────────────────┘\
""",
    ])


def test_apply_lenses_multiple_outputs_preserves_other_columns() -> None:
    """Unrelated columns are forwarded onto each of a multi-output lens's chunks."""
    accel = pa.StructArray.from_arrays(
        [pa.array([1.0]), pa.array([2.0])],
        names=["x", "y"],
    )
    temperature = pa.array([42.0])
    chunk = Chunk.from_columns(
        "/sensor",
        indexes=[rr.TimeColumn("frame", sequence=[0])],
        columns=rr.DynamicArchetype.columns(
            archetype="Imu",
            components={"accel": accel, "temperature": temperature},
        ),
    )

    assert chunk.format(redact=True) == inline_snapshot("""\
┌─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                                                                           │
│ * entity_path: /sensor                                                                                                                              │
│ * id: [**REDACTED**]                                                                                                                                │
│ * version: [**REDACTED**]                                                                                                                           │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌───────────────────────────────────────────────┬───────────────────┬────────────────────────────────────────────────┬────────────────────────────┐ │
│ │ RowId                                         ┆ frame             ┆ Imu:accel                                      ┆ Imu:temperature            │ │
│ │ ---                                           ┆ ---               ┆ ---                                            ┆ ---                        │ │
│ │ type: non-null FixedSizeBinary(16)            ┆ type: Int64       ┆ type: List(Struct("x": Float64, "y": Float64)) ┆ type: List(Float64)        │ │
│ │ ARROW:extension:metadata: {"namespace":"row"} ┆ index_name: frame ┆ archetype: Imu                                 ┆ archetype: Imu             │ │
│ │ ARROW:extension:name: TUID                    ┆ is_sorted: true   ┆ component: Imu:accel                           ┆ component: Imu:temperature │ │
│ │ is_sorted: true                               ┆ kind: index       ┆ kind: data                                     ┆ kind: data                 │ │
│ │ kind: control                                 ┆                   ┆                                                ┆                            │ │
│ ╞═══════════════════════════════════════════════╪═══════════════════╪════════════════════════════════════════════════╪════════════════════════════╡ │
│ │ row_[**REDACTED**]                            ┆ 0                 ┆ [{x: 1.0, y: 2.0}]                             ┆ [42.0]                     │ │
│ └───────────────────────────────────────────────┴───────────────────┴────────────────────────────────────────────────┴────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘\
""")

    lens = Lens(
        "Imu:accel",
        to_entity={
            "/out/x": LensOutput().to_component(rr.Scalars.descriptor_scalars(), ".x"),
            "/out/y": LensOutput().to_component(rr.Scalars.descriptor_scalars(), ".y"),
        },
    )
    results = chunk.apply_lenses(lens)

    # The original chunk should not be forwarded as is, so it's id must not be visible here
    assert chunk.id not in {r.id for r in results}
    assert [r.format(redact=True) for r in results] == inline_snapshot([
        """\
┌────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                          │
│ * entity_path: /sensor                                                                             │
│ * id: [**REDACTED**]                                                                               │
│ * version: [**REDACTED**]                                                                          │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌───────────────────────────────────────────────┬───────────────────┬────────────────────────────┐ │
│ │ RowId                                         ┆ frame             ┆ Imu:temperature            │ │
│ │ ---                                           ┆ ---               ┆ ---                        │ │
│ │ type: non-null FixedSizeBinary(16)            ┆ type: Int64       ┆ type: List(Float64)        │ │
│ │ ARROW:extension:metadata: {"namespace":"row"} ┆ index_name: frame ┆ archetype: Imu             │ │
│ │ ARROW:extension:name: TUID                    ┆ is_sorted: true   ┆ component: Imu:temperature │ │
│ │ is_sorted: true                               ┆ kind: index       ┆ kind: data                 │ │
│ │ kind: control                                 ┆                   ┆                            │ │
│ ╞═══════════════════════════════════════════════╪═══════════════════╪════════════════════════════╡ │
│ │ row_[**REDACTED**]                            ┆ 0                 ┆ [42.0]                     │ │
│ └───────────────────────────────────────────────┴───────────────────┴────────────────────────────┘ │
└────────────────────────────────────────────────────────────────────────────────────────────────────┘\
""",
        """\
┌────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                          │
│ * entity_path: /out/x                                                                              │
│ * id: [**REDACTED**]                                                                               │
│ * version: [**REDACTED**]                                                                          │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌───────────────────────────────────────────────┬───────────────────┬────────────────────────────┐ │
│ │ RowId                                         ┆ frame             ┆ Scalars:scalars            │ │
│ │ ---                                           ┆ ---               ┆ ---                        │ │
│ │ type: non-null FixedSizeBinary(16)            ┆ type: Int64       ┆ type: List(Float64)        │ │
│ │ ARROW:extension:metadata: {"namespace":"row"} ┆ index_name: frame ┆ archetype: Scalars         │ │
│ │ ARROW:extension:name: TUID                    ┆ is_sorted: true   ┆ component: Scalars:scalars │ │
│ │ is_sorted: true                               ┆ kind: index       ┆ component_type: Scalar     │ │
│ │ kind: control                                 ┆                   ┆ kind: data                 │ │
│ ╞═══════════════════════════════════════════════╪═══════════════════╪════════════════════════════╡ │
│ │ row_[**REDACTED**]                            ┆ 0                 ┆ [1.0]                      │ │
│ └───────────────────────────────────────────────┴───────────────────┴────────────────────────────┘ │
└────────────────────────────────────────────────────────────────────────────────────────────────────┘\
""",
        """\
┌────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                          │
│ * entity_path: /out/y                                                                              │
│ * id: [**REDACTED**]                                                                               │
│ * version: [**REDACTED**]                                                                          │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌───────────────────────────────────────────────┬───────────────────┬────────────────────────────┐ │
│ │ RowId                                         ┆ frame             ┆ Scalars:scalars            │ │
│ │ ---                                           ┆ ---               ┆ ---                        │ │
│ │ type: non-null FixedSizeBinary(16)            ┆ type: Int64       ┆ type: List(Float64)        │ │
│ │ ARROW:extension:metadata: {"namespace":"row"} ┆ index_name: frame ┆ archetype: Scalars         │ │
│ │ ARROW:extension:name: TUID                    ┆ is_sorted: true   ┆ component: Scalars:scalars │ │
│ │ is_sorted: true                               ┆ kind: index       ┆ component_type: Scalar     │ │
│ │ kind: control                                 ┆                   ┆ kind: data                 │ │
│ ╞═══════════════════════════════════════════════╪═══════════════════╪════════════════════════════╡ │
│ │ row_[**REDACTED**]                            ┆ 0                 ┆ [2.0]                      │ │
│ └───────────────────────────────────────────────┴───────────────────┴────────────────────────────┘ │
└────────────────────────────────────────────────────────────────────────────────────────────────────┘\
""",
    ])


def test_apply_lenses_time_extraction() -> None:
    """apply_lenses can extract a time column from struct data."""
    data = pa.StructArray.from_arrays(
        [
            pa.array([1.0, 2.0], type=pa.float64()),
            pa.array([1_000_000_000, 2_000_000_000], type=pa.int64()),
        ],
        names=["value", "ts"],
    )
    chunk = Chunk.from_columns(
        "/sensor",
        indexes=[rr.TimeColumn("frame", sequence=[0, 1])],
        columns=rr.DynamicArchetype.columns(archetype="Sensor", components={"data": data}),
    )

    assert chunk.format(redact=True) == inline_snapshot("""\
┌───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                                                 │
│ * entity_path: /sensor                                                                                                    │
│ * id: [**REDACTED**]                                                                                                      │
│ * version: [**REDACTED**]                                                                                                 │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌───────────────────────────────────────────────┬───────────────────┬───────────────────────────────────────────────────┐ │
│ │ RowId                                         ┆ frame             ┆ Sensor:data                                       │ │
│ │ ---                                           ┆ ---               ┆ ---                                               │ │
│ │ type: non-null FixedSizeBinary(16)            ┆ type: Int64       ┆ type: List(Struct("value": Float64, "ts": Int64)) │ │
│ │ ARROW:extension:metadata: {"namespace":"row"} ┆ index_name: frame ┆ archetype: Sensor                                 │ │
│ │ ARROW:extension:name: TUID                    ┆ is_sorted: true   ┆ component: Sensor:data                            │ │
│ │ is_sorted: true                               ┆ kind: index       ┆ kind: data                                        │ │
│ │ kind: control                                 ┆                   ┆                                                   │ │
│ ╞═══════════════════════════════════════════════╪═══════════════════╪═══════════════════════════════════════════════════╡ │
│ │ row_[**REDACTED**]                            ┆ 0                 ┆ [{value: 1.0, ts: 1000000000}]                    │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ row_[**REDACTED**]                            ┆ 1                 ┆ [{value: 2.0, ts: 2000000000}]                    │ │
│ └───────────────────────────────────────────────┴───────────────────┴───────────────────────────────────────────────────┘ │
└───────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘\
""")

    lens = Lens(
        "Sensor:data",
        LensOutput()
        .to_component(rr.Scalars.descriptor_scalars(), ".value")
        .to_timeline("sensor_time", "timestamp_ns", ".ts"),
    )
    results = chunk.apply_lenses(lens)

    assert len(results) == 1
    assert chunk.id != results[0].id
    assert results[0].format(redact=True) == inline_snapshot("""\
┌──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                                                    │
│ * entity_path: /sensor                                                                                                       │
│ * id: [**REDACTED**]                                                                                                         │
│ * version: [**REDACTED**]                                                                                                    │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌───────────────────────────────────────────────┬───────────────────┬─────────────────────────┬────────────────────────────┐ │
│ │ RowId                                         ┆ frame             ┆ sensor_time             ┆ Scalars:scalars            │ │
│ │ ---                                           ┆ ---               ┆ ---                     ┆ ---                        │ │
│ │ type: non-null FixedSizeBinary(16)            ┆ type: Int64       ┆ type: Timestamp(ns)     ┆ type: List(Float64)        │ │
│ │ ARROW:extension:metadata: {"namespace":"row"} ┆ index_name: frame ┆ index_name: sensor_time ┆ archetype: Scalars         │ │
│ │ ARROW:extension:name: TUID                    ┆ is_sorted: true   ┆ is_sorted: true         ┆ component: Scalars:scalars │ │
│ │ is_sorted: true                               ┆ kind: index       ┆ kind: index             ┆ component_type: Scalar     │ │
│ │ kind: control                                 ┆                   ┆                         ┆ kind: data                 │ │
│ ╞═══════════════════════════════════════════════╪═══════════════════╪═════════════════════════╪════════════════════════════╡ │
│ │ row_[**REDACTED**]                            ┆ 0                 ┆ 1970-01-01T00:00:01     ┆ [1.0]                      │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ row_[**REDACTED**]                            ┆ 1                 ┆ 1970-01-01T00:00:02     ┆ [2.0]                      │ │
│ └───────────────────────────────────────────────┴───────────────────┴─────────────────────────┴────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘\
""")


def test_apply_lenses_with_pipe() -> None:
    """apply_lenses works with Selector.pipe() for Python-side transforms."""
    import pyarrow.compute as pc

    data = pa.StructArray.from_arrays(
        [pa.array([1.0, 2.0], type=pa.float64())],
        names=["x"],
    )
    chunk = Chunk.from_columns(
        "/sensor",
        indexes=[rr.TimeColumn("frame", sequence=[0, 1])],
        columns=rr.DynamicArchetype.columns(archetype="S", components={"d": data}),
    )

    assert chunk.format(redact=True) == inline_snapshot("""\
┌──────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                                │
│ * entity_path: /sensor                                                                                   │
│ * id: [**REDACTED**]                                                                                     │
│ * version: [**REDACTED**]                                                                                │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌───────────────────────────────────────────────┬───────────────────┬──────────────────────────────────┐ │
│ │ RowId                                         ┆ frame             ┆ S:d                              │ │
│ │ ---                                           ┆ ---               ┆ ---                              │ │
│ │ type: non-null FixedSizeBinary(16)            ┆ type: Int64       ┆ type: List(Struct("x": Float64)) │ │
│ │ ARROW:extension:metadata: {"namespace":"row"} ┆ index_name: frame ┆ archetype: S                     │ │
│ │ ARROW:extension:name: TUID                    ┆ is_sorted: true   ┆ component: S:d                   │ │
│ │ is_sorted: true                               ┆ kind: index       ┆ kind: data                       │ │
│ │ kind: control                                 ┆                   ┆                                  │ │
│ ╞═══════════════════════════════════════════════╪═══════════════════╪══════════════════════════════════╡ │
│ │ row_[**REDACTED**]                            ┆ 0                 ┆ [{x: 1.0}]                       │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ row_[**REDACTED**]                            ┆ 1                 ┆ [{x: 2.0}]                       │ │
│ └───────────────────────────────────────────────┴───────────────────┴──────────────────────────────────┘ │
└──────────────────────────────────────────────────────────────────────────────────────────────────────────┘\
""")

    selector = Selector(".x").pipe(lambda arr: pc.multiply(arr, 2.0))
    lens = Lens(
        "S:d",
        LensOutput().to_component(rr.Scalars.descriptor_scalars(), selector),
    )
    results = chunk.apply_lenses(lens)

    assert len(results) == 1
    assert chunk.id != results[0].id
    assert results[0].format(redact=True) == inline_snapshot("""\
┌────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                          │
│ * entity_path: /sensor                                                                             │
│ * id: [**REDACTED**]                                                                               │
│ * version: [**REDACTED**]                                                                          │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌───────────────────────────────────────────────┬───────────────────┬────────────────────────────┐ │
│ │ RowId                                         ┆ frame             ┆ Scalars:scalars            │ │
│ │ ---                                           ┆ ---               ┆ ---                        │ │
│ │ type: non-null FixedSizeBinary(16)            ┆ type: Int64       ┆ type: List(Float64)        │ │
│ │ ARROW:extension:metadata: {"namespace":"row"} ┆ index_name: frame ┆ archetype: Scalars         │ │
│ │ ARROW:extension:name: TUID                    ┆ is_sorted: true   ┆ component: Scalars:scalars │ │
│ │ is_sorted: true                               ┆ kind: index       ┆ component_type: Scalar     │ │
│ │ kind: control                                 ┆                   ┆ kind: data                 │ │
│ ╞═══════════════════════════════════════════════╪═══════════════════╪════════════════════════════╡ │
│ │ row_[**REDACTED**]                            ┆ 0                 ┆ [2.0]                      │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ row_[**REDACTED**]                            ┆ 1                 ┆ [4.0]                      │ │
│ └───────────────────────────────────────────────┴───────────────────┴────────────────────────────┘ │
└────────────────────────────────────────────────────────────────────────────────────────────────────┘\
""")


# ---------------------------------------------------------------------------
# apply_selector
# ---------------------------------------------------------------------------


def test_apply_selector_doubles_values() -> None:
    """apply_selector doubles float values via pipe, keeping other columns intact."""
    import pyarrow.compute as pc

    chunk = Chunk.from_columns(
        "/sensor",
        indexes=[rr.TimeColumn("tick", sequence=[0, 1])],
        columns=rr.DynamicArchetype.columns(
            archetype="MyArchetype", components={"value": pa.array([1.0, 2.0], type=pa.float64())}
        ),
    )

    assert chunk.format(redact=True) == inline_snapshot("""\
┌─────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                           │
│ * entity_path: /sensor                                                                              │
│ * id: [**REDACTED**]                                                                                │
│ * version: [**REDACTED**]                                                                           │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌───────────────────────────────────────────────┬──────────────────┬──────────────────────────────┐ │
│ │ RowId                                         ┆ tick             ┆ MyArchetype:value            │ │
│ │ ---                                           ┆ ---              ┆ ---                          │ │
│ │ type: non-null FixedSizeBinary(16)            ┆ type: Int64      ┆ type: List(Float64)          │ │
│ │ ARROW:extension:metadata: {"namespace":"row"} ┆ index_name: tick ┆ archetype: MyArchetype       │ │
│ │ ARROW:extension:name: TUID                    ┆ is_sorted: true  ┆ component: MyArchetype:value │ │
│ │ is_sorted: true                               ┆ kind: index      ┆ kind: data                   │ │
│ │ kind: control                                 ┆                  ┆                              │ │
│ ╞═══════════════════════════════════════════════╪══════════════════╪══════════════════════════════╡ │
│ │ row_[**REDACTED**]                            ┆ 0                ┆ [1.0]                        │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ row_[**REDACTED**]                            ┆ 1                ┆ [2.0]                        │ │
│ └───────────────────────────────────────────────┴──────────────────┴──────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────────────────────────────┘\
""")

    selector = Selector(".").pipe(lambda arr: pc.multiply(arr, 2.0))
    result = chunk.apply_selector("MyArchetype:value", selector)

    assert isinstance(result, Chunk)
    assert result.num_rows == chunk.num_rows
    assert result.entity_path == "/sensor"
    assert chunk.id != result.id
    assert result.format(redact=True) == inline_snapshot("""\
┌─────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ METADATA:                                                                                           │
│ * entity_path: /sensor                                                                              │
│ * id: [**REDACTED**]                                                                                │
│ * version: [**REDACTED**]                                                                           │
├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤
│ ┌───────────────────────────────────────────────┬──────────────────┬──────────────────────────────┐ │
│ │ RowId                                         ┆ tick             ┆ MyArchetype:value            │ │
│ │ ---                                           ┆ ---              ┆ ---                          │ │
│ │ type: non-null FixedSizeBinary(16)            ┆ type: Int64      ┆ type: List(Float64)          │ │
│ │ ARROW:extension:metadata: {"namespace":"row"} ┆ index_name: tick ┆ archetype: MyArchetype       │ │
│ │ ARROW:extension:name: TUID                    ┆ is_sorted: true  ┆ component: MyArchetype:value │ │
│ │ is_sorted: true                               ┆ kind: index      ┆ kind: data                   │ │
│ │ kind: control                                 ┆                  ┆                              │ │
│ ╞═══════════════════════════════════════════════╪══════════════════╪══════════════════════════════╡ │
│ │ row_[**REDACTED**]                            ┆ 0                ┆ [2.0]                        │ │
│ ├╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┼╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌┤ │
│ │ row_[**REDACTED**]                            ┆ 1                ┆ [4.0]                        │ │
│ └───────────────────────────────────────────────┴──────────────────┴──────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────────────────────────────────────────┘\
""")


def test_apply_selector_component_not_found() -> None:
    """apply_selector raises ValueError when the source component doesn't exist."""
    chunk = Chunk.from_columns(
        "/test",
        indexes=[rr.TimeColumn("tick", sequence=[0])],
        columns=rr.DynamicArchetype.columns(
            archetype="MyArchetype", components={"value": pa.array([1.0], type=pa.float64())}
        ),
    )

    with pytest.raises(ValueError, match="not found"):
        chunk.apply_selector("nonexistent:component", Selector("."))

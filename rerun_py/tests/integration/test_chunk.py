"""Tests for Chunk construction from PyArrow data."""

from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa
import pytest
import rerun as rr
from inline_snapshot import snapshot as inline_snapshot
from rerun.experimental import Chunk, RrdReader

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

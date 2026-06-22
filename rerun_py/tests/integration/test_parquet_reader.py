"""
Tests for rerun.experimental.ParquetReader.

The reader turns raw parquet columns into grouped, time-indexed `Chunk`s — prefix /
individual / explicit-prefix grouping, index columns with unit scaling, static
columns, and error paths. Mapping the resulting struct components into archetypes is
done separately with lenses (see `test_lazy_chunk_stream.py`).
"""

from __future__ import annotations

import itertools
from typing import TYPE_CHECKING

import numpy as np
import pyarrow as pa
import pyarrow.compute as pc
import pyarrow.parquet as pq
import pytest
import rerun as rr
from rerun.experimental import Chunk, DeriveLens, ParquetReader, Selector, StreamingReader

if TYPE_CHECKING:
    from collections.abc import Callable
    from pathlib import Path

    ParquetWriter = Callable[[dict[str, pa.Array]], Path]


# ---------------------------------------------------------------------------
# Fixtures / helpers
# ---------------------------------------------------------------------------


@pytest.fixture
def parquet_writer(tmp_path: Path) -> ParquetWriter:
    """Return a callable that writes named Arrow columns to a fresh parquet file and returns its path."""
    counter = itertools.count()

    def write(columns: dict[str, pa.Array]) -> Path:
        path = tmp_path / f"t{next(counter)}.parquet"
        pq.write_table(pa.table(columns), str(path))
        return path

    return write


def _data_chunks(reader: ParquetReader) -> list[Chunk]:
    """Run the reader, dropping the file-metadata `/__properties` chunk that parquet's schema metadata produces."""
    return reader.stream().drop(content="/__properties/**").to_chunks()


def _by_entity(chunks: list[Chunk], entity_path: str) -> Chunk:
    matches = [c for c in chunks if c.entity_path == entity_path]
    assert len(matches) == 1, f"expected exactly one chunk at {entity_path}, found {len(matches)}"
    return matches[0]


def _struct_field_names(chunk: Chunk, component: str = "data") -> list[str]:
    """Field names of a `List<Struct>` component."""
    return list(chunk.to_record_batch().schema.field(component).type.value_type.names)


# TODO(RR-4935): use selector function when available
def _struct_fields_to_fsl(struct_arr: pa.StructArray, fields: list[str]) -> pa.FixedSizeListArray:
    """
    Interleave named struct fields (cast to f32) row-wise into a `FixedSizeList(len(fields), f32)`.

    This is the kind of one-off helper a user writes today to drive a lens; a built-in
    `DeriveLens` archetype helper will make it unnecessary once helpers are reintroduced.
    """
    columns = [pc.cast(struct_arr.field(f), pa.float32()).to_numpy(zero_copy_only=False) for f in fields]
    flat = pa.array(np.stack(columns, axis=1).reshape(-1), type=pa.float32())
    return pa.FixedSizeListArray.from_arrays(
        flat, type=pa.list_(pa.field("item", pa.float32(), nullable=False), len(fields))
    )


# ---------------------------------------------------------------------------
# prefix grouping
# ---------------------------------------------------------------------------


def test_prefix_grouping(parquet_writer: ParquetWriter) -> None:
    """Multi-column prefixes become a single `data` struct; a lone column becomes a raw component."""

    # Prefix grouping (delimiter `_`) yields:
    # - `A_*`      → entity `/A`,      struct `data{pos_x..quat_w}`
    # - `obs_*`    → entity `/obs`,    struct `data{x, y, z}`
    # - `camera_*` → entity `/camera`, struct `data{rgb, depth}`
    # - `speed`    → entity `/speed`,  a raw `speed` component (no delimiter → lone column)
    path = parquet_writer({
        "frame_index": pa.array([0, 1, 2], pa.int64()),
        "A_pos_x": pa.array([1.0, 2.0, 3.0]),
        "A_pos_y": pa.array([4.0, 5.0, 6.0]),
        "A_pos_z": pa.array([7.0, 8.0, 9.0]),
        "A_quat_x": pa.array([0.0, 0.0, 0.0]),
        "A_quat_y": pa.array([0.0, 0.0, 0.0]),
        "A_quat_z": pa.array([0.0, 0.0, 0.0]),
        "A_quat_w": pa.array([1.0, 1.0, 1.0]),
        "obs_x": pa.array([1.0, 2.0, 3.0]),
        "obs_y": pa.array([4.0, 5.0, 6.0]),
        "obs_z": pa.array([7.0, 8.0, 9.0]),
        "camera_rgb": pa.array([10.0, 20.0, 30.0]),
        "camera_depth": pa.array([40.0, 50.0, 60.0]),
        "speed": pa.array([100.0, 200.0, 300.0]),
    })
    chunks = _data_chunks(ParquetReader(path, index_columns=[("frame_index", "sequence")]))

    assert {c.entity_path for c in chunks} == {"/A", "/obs", "/camera", "/speed"}

    camera = _by_entity(chunks, "/camera")
    assert camera.num_rows == 3
    assert camera.timeline_names == ["frame_index"]
    assert _struct_field_names(camera) == ["rgb", "depth"]
    assert camera.to_record_batch().column("data").to_pylist() == [
        [{"rgb": 10.0, "depth": 40.0}],
        [{"rgb": 20.0, "depth": 50.0}],
        [{"rgb": 30.0, "depth": 60.0}],
    ]

    assert _struct_field_names(_by_entity(chunks, "/obs")) == ["x", "y", "z"]
    assert _struct_field_names(_by_entity(chunks, "/A")) == [
        "pos_x",
        "pos_y",
        "pos_z",
        "quat_x",
        "quat_y",
        "quat_z",
        "quat_w",
    ]

    # Lone column → its own raw component named after the column (not a `data` struct).
    speed = _by_entity(chunks, "/speed")
    assert "data" not in speed.to_record_batch().schema.names
    assert speed.to_record_batch().column("speed").to_pylist() == [[100.0], [200.0], [300.0]]


def test_individual_grouping(parquet_writer: ParquetWriter) -> None:
    """Individual grouping gives every column its own entity/component — no struct packing."""
    path = parquet_writer({
        "frame_index": pa.array([0, 1, 2], pa.int64()),
        "camera_rgb": pa.array([1.0, 2.0, 3.0]),
        "camera_depth": pa.array([4.0, 5.0, 6.0]),
    })
    chunks = _data_chunks(
        ParquetReader(path, column_grouping="individual", index_columns=[("frame_index", "sequence")])
    )
    assert {c.entity_path for c in chunks} == {"/camera_rgb", "/camera_depth"}
    for c in chunks:
        assert "data" not in c.to_record_batch().schema.names


def test_explicit_prefixes(parquet_writer: ParquetWriter) -> None:
    """Explicit prefixes group by exact prefix string; unmatched columns become individual groups."""
    path = parquet_writer({
        "fooa": pa.array([1.0, 2.0]),
        "foob": pa.array([3.0, 4.0]),
        "cata": pa.array([5.0, 6.0]),
        "catb": pa.array([7.0, 8.0]),
        "other": pa.array([9.0, 10.0]),
    })
    chunks = _data_chunks(ParquetReader(path, column_grouping="explicit_prefixes", prefixes=["cat", "foo"]))
    assert {c.entity_path for c in chunks} == {"/foo", "/cat", "/other"}
    # The prefix is stripped from each struct field name.
    assert _struct_field_names(_by_entity(chunks, "/foo")) == ["a", "b"]
    assert _struct_field_names(_by_entity(chunks, "/cat")) == ["a", "b"]


# ---------------------------------------------------------------------------
# index columns
# ---------------------------------------------------------------------------


def test_index_sequence(parquet_writer: ParquetWriter) -> None:
    path = parquet_writer({"frame_index": pa.array([0, 1, 2], pa.int64()), "value": pa.array([10.0, 20.0, 30.0])})
    chunk = _by_entity(
        _data_chunks(ParquetReader(path, index_columns=[("frame_index", "sequence")])),
        "/value",
    )
    rb = chunk.to_record_batch()
    assert rb.schema.field("frame_index").type == pa.int64()
    assert rb.column("frame_index").to_pylist() == [0, 1, 2]


def test_index_timestamp_unit_scaling(parquet_writer: ParquetWriter) -> None:
    """A `ms` timestamp index is scaled to nanoseconds and typed `timestamp[ns]`."""
    path = parquet_writer({"ts_ms": pa.array([1, 2, 3], pa.int64()), "value": pa.array([1.0, 2.0, 3.0])})
    chunk = _by_entity(
        _data_chunks(ParquetReader(path, index_columns=[("ts_ms", "timestamp", "ms")])),
        "/value",
    )
    rb = chunk.to_record_batch()
    assert rb.schema.field("ts_ms").type == pa.timestamp("ns")
    assert rb.column("ts_ms").cast(pa.int64()).to_pylist() == [1_000_000, 2_000_000, 3_000_000]


def test_index_duration_unit_scaling(parquet_writer: ParquetWriter) -> None:
    """A `us` duration index is scaled to nanoseconds and typed `duration[ns]`."""
    path = parquet_writer({"elapsed_us": pa.array([100, 200, 300], pa.int64()), "value": pa.array([1.0, 2.0, 3.0])})
    chunk = _by_entity(
        _data_chunks(ParquetReader(path, index_columns=[("elapsed_us", "duration", "us")])),
        "/value",
    )
    rb = chunk.to_record_batch()
    assert rb.schema.field("elapsed_us").type == pa.duration("ns")
    assert rb.column("elapsed_us").cast(pa.int64()).to_pylist() == [100_000, 200_000, 300_000]


# ---------------------------------------------------------------------------
# static columns
# ---------------------------------------------------------------------------


def test_static_columns(parquet_writer: ParquetWriter) -> None:
    """Uniform static columns are emitted once as a separate static chunk."""
    path = parquet_writer({
        "frame_index": pa.array([0, 1, 2], pa.int64()),
        "value": pa.array([1.0, 2.0, 3.0]),
        "suite": pa.array(["s", "s", "s"]),
        "agg": pa.array(["mean", "mean", "mean"]),
    })
    chunks = _data_chunks(
        ParquetReader(
            path,
            column_grouping="individual",
            index_columns=[("frame_index", "sequence")],
            static_columns=["suite", "agg"],
        )
    )
    static = [c for c in chunks if c.is_static]
    assert len(static) == 1
    assert static[0].num_rows == 1
    assert {c for c in static[0].to_record_batch().schema.names if not c.startswith("rerun.controls")} == {
        "suite",
        "agg",
    }

    temporal = [c for c in chunks if not c.is_static]
    assert {c.entity_path for c in temporal} == {"/value"}


def test_static_column_non_uniform_is_error(parquet_writer: ParquetWriter) -> None:
    """A static column with varying values raises when the stream runs."""
    path = parquet_writer({"x": pa.array([1.0, 2.0]), "suite": pa.array(["a", "b"])})
    with pytest.raises(Exception, match=r"non-uniform|static"):
        ParquetReader(path, column_grouping="individual", static_columns=["suite"]).stream().to_chunks()


# ---------------------------------------------------------------------------
# error paths
# ---------------------------------------------------------------------------


def test_file_not_found(tmp_path: Path) -> None:
    with pytest.raises(FileNotFoundError, match="not found"):
        ParquetReader(tmp_path / "nonexistent.parquet")


def test_invalid_column_grouping(parquet_writer: ParquetWriter) -> None:
    path = parquet_writer({"x": pa.array([1.0])})
    with pytest.raises(ValueError, match="Unknown column_grouping"):
        ParquetReader(path, column_grouping="bogus")


def test_prefixes_without_explicit_grouping_is_error(parquet_writer: ParquetWriter) -> None:
    path = parquet_writer({"x": pa.array([1.0])})
    with pytest.raises(ValueError, match="explicit_prefixes"):
        ParquetReader(path, prefixes=["foo"])


def test_missing_index_column_is_error(parquet_writer: ParquetWriter) -> None:
    path = parquet_writer({"x": pa.array([1.0])})
    with pytest.raises(Exception, match="not found"):
        ParquetReader(path, index_columns=[("missing", "sequence")]).stream().to_chunks()


# ---------------------------------------------------------------------------
# Archetype mapping via lenses
# ---------------------------------------------------------------------------


def test_transform3d_via_lenses(parquet_writer: ParquetWriter) -> None:
    """
    Reproduce the old `ColumnRule` mapping — a `Transform3D` (translation + rotation) — with lenses.

    The lens is verbose for now (it interleaves the struct fields by hand via `_struct_fields_to_fsl`);
    a `DeriveLens` archetype helper will collapse this to a one-liner once helpers are reintroduced.
    """
    # A pose table: per-row translation (`pos_*`) and rotation quaternion (`quat_*`) under prefix `A`.
    path = parquet_writer({
        "frame_index": pa.array([0, 1], pa.int64()),
        "A_pos_x": pa.array([1.0, 2.0]),
        "A_pos_y": pa.array([3.0, 4.0]),
        "A_pos_z": pa.array([5.0, 6.0]),
        "A_quat_x": pa.array([0.0, 0.0]),
        "A_quat_y": pa.array([0.0, 0.0]),
        "A_quat_z": pa.array([0.0, 0.0]),
        "A_quat_w": pa.array([1.0, 1.0]),
    })

    # Read the `pos_*` / `quat_*` fields off the reader's `data` struct and interleave them
    # into the `FixedSizeList<f32>` arrays the Transform3D components expect.
    # TODO(RR-4935): use selector function when available
    lens = (
        DeriveLens("data", output_entity="/pose")
        .to_component(
            rr.Transform3D.descriptor_translation(),
            Selector(".").pipe(lambda s: _struct_fields_to_fsl(s, ["pos_x", "pos_y", "pos_z"])),
        )
        .to_component(
            rr.Transform3D.descriptor_quaternion(),
            Selector(".").pipe(lambda s: _struct_fields_to_fsl(s, ["quat_x", "quat_y", "quat_z", "quat_w"])),
        )
    )

    chunks = (
        ParquetReader(path, index_columns=[("frame_index", "sequence")])
        .stream()
        .lenses([lens], content="/A", output_mode="drop_unmatched")
        .to_chunks()
    )
    pose = _by_entity(chunks, "/pose")
    rb = pose.to_record_batch()

    # The emitted Arrow types match the real Transform3D components exactly.
    vec3d = rr.Transform3D(translation=[[0, 0, 0]]).as_component_batches()[0].as_arrow_array().type
    quat = rr.Transform3D(quaternion=rr.Quaternion(xyzw=[0.0, 0.0, 0.0, 1.0])).as_component_batches()[0]
    translation = rb.column("Transform3D:translation")
    quaternion = rb.column("Transform3D:quaternion")
    assert translation.type.value_type == vec3d
    assert quaternion.type.value_type == quat.as_arrow_array().type

    # Values interleaved row-major from the source columns; timeline preserved.
    assert translation.to_pylist() == [[[1.0, 3.0, 5.0]], [[2.0, 4.0, 6.0]]]
    assert quaternion.to_pylist() == [[[0.0, 0.0, 0.0, 1.0]], [[0.0, 0.0, 0.0, 1.0]]]
    assert rb.column("frame_index").to_pylist() == [0, 1]


# ---------------------------------------------------------------------------
# StreamingReader protocol conformance
# ---------------------------------------------------------------------------


def test_streaming_reader_protocol(parquet_writer: ParquetWriter) -> None:
    path = parquet_writer({"x": pa.array([1.0])})
    assert isinstance(ParquetReader(path), StreamingReader)

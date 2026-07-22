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

import pyarrow as pa
import pyarrow.parquet as pq
import pytest
import rerun as rr
from rerun.experimental import Chunk, DeriveLens, IndexColumn, LazyChunkStream, ParquetReader, StreamingReader

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
    chunks = _data_chunks(ParquetReader(path, index_columns=[IndexColumn.sequence("frame_index")]))

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
        ParquetReader(path, column_grouping="individual", index_columns=[IndexColumn.sequence("frame_index")])
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
        _data_chunks(ParquetReader(path, index_columns=[IndexColumn.sequence("frame_index")])),
        "/value",
    )
    rb = chunk.to_record_batch()
    assert rb.schema.field("frame_index").type == pa.int64()
    assert rb.column("frame_index").to_pylist() == [0, 1, 2]


def test_index_timestamp_unit_scaling(parquet_writer: ParquetWriter) -> None:
    """A `ms` timestamp index is scaled to nanoseconds and typed `timestamp[ns]`."""
    path = parquet_writer({"ts_ms": pa.array([1, 2, 3], pa.int64()), "value": pa.array([1.0, 2.0, 3.0])})
    chunk = _by_entity(
        _data_chunks(ParquetReader(path, index_columns=[IndexColumn.timestamp("ts_ms", input_unit="ms")])),
        "/value",
    )
    rb = chunk.to_record_batch()
    assert rb.schema.field("ts_ms").type == pa.timestamp("ns")
    assert rb.column("ts_ms").cast(pa.int64()).to_pylist() == [1_000_000, 2_000_000, 3_000_000]


def test_index_duration_unit_scaling(parquet_writer: ParquetWriter) -> None:
    """A `us` duration index is scaled to nanoseconds and typed `duration[ns]`."""
    path = parquet_writer({"elapsed_us": pa.array([100, 200, 300], pa.int64()), "value": pa.array([1.0, 2.0, 3.0])})
    chunk = _by_entity(
        _data_chunks(ParquetReader(path, index_columns=[IndexColumn.duration("elapsed_us", input_unit="us")])),
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
            index_columns=[IndexColumn.sequence("frame_index")],
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
        ParquetReader(path, index_columns=[IndexColumn.sequence("missing")]).stream().to_chunks()


# ---------------------------------------------------------------------------
# Archetype mapping via lenses
# ---------------------------------------------------------------------------


def test_transform3d_via_lenses(parquet_writer: ParquetWriter) -> None:
    """
    Reproduce the old `ColumnRule` mapping — a `Transform3D` (translation + rotation) — with lens helpers.

    `to_translation` / `to_quaternion` pack the reader's `data` struct fields and cast them to the
    `FixedSizeList<f32>` arrays the Transform3D components expect; chaining them on one lens builds a
    full transform. This also exercises the FSL→FSL `f64`→`f32` auto-cast end to end.
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

    lens = (
        DeriveLens("data", output_entity="/pose")
        .to_translation("pos_x", "pos_y", "pos_z")
        .to_quaternion("quat_x", "quat_y", "quat_z", "quat_w")
    )

    chunks = (
        ParquetReader(path, index_columns=[IndexColumn.sequence("frame_index")])
        .stream()
        .lenses([lens], content="/A", output_mode="drop_unmatched")
        .to_chunks()
    )
    pose = _by_entity(chunks, "/pose")
    rb = pose.to_record_batch()

    # The emitted Arrow types match the real Transform3D components exactly (incl. the f32 cast).
    translation = rb.column("Transform3D:translation")
    quaternion = rb.column("Transform3D:quaternion")
    assert translation.type.value_type == rr.components.Translation3D.arrow_type()
    assert quaternion.type.value_type == rr.components.RotationQuat.arrow_type()

    # Values packed row-major from the source columns; timeline preserved.
    assert translation.to_pylist() == [[[1.0, 3.0, 5.0]], [[2.0, 4.0, 6.0]]]
    assert quaternion.to_pylist() == [[[0.0, 0.0, 0.0, 1.0]], [[0.0, 0.0, 0.0, 1.0]]]
    assert rb.column("frame_index").to_pylist() == [0, 1]


def test_to_packed_component_generic(parquet_writer: ParquetWriter) -> None:
    """The generic `to_packed_component` maps struct fields onto an arbitrary fixed-size-list component."""
    # Prefix `p` → entity `/p`, struct `data{x, y, z}`.
    path = parquet_writer({
        "frame_index": pa.array([0, 1], pa.int64()),
        "p_x": pa.array([1.0, 2.0]),
        "p_y": pa.array([3.0, 4.0]),
        "p_z": pa.array([5.0, 6.0]),
    })

    lens = DeriveLens("data", output_entity="/points").to_packed_component(
        rr.Points3D.descriptor_positions(), "x", "y", "z"
    )

    chunks = (
        ParquetReader(path, index_columns=[IndexColumn.sequence("frame_index")])
        .stream()
        .lenses([lens], content="/p", output_mode="drop_unmatched")
        .to_chunks()
    )
    positions = _by_entity(chunks, "/points").to_record_batch().column("Points3D:positions")

    assert positions.type.value_type == rr.components.Position3D.arrow_type()
    assert positions.to_pylist() == [[[1.0, 3.0, 5.0]], [[2.0, 4.0, 6.0]]]


def test_to_rotation_axis_angle(parquet_writer: ParquetWriter) -> None:
    """`to_rotation_axis_angle` builds the `Struct{axis, angle}` a `RotationAxisAngle` expects."""
    # Prefix `r` → entity `/r`, struct `data{ax, ay, az, angle}`.
    path = parquet_writer({
        "frame_index": pa.array([0, 1], pa.int64()),
        "r_ax": pa.array([1.0, 0.0]),
        "r_ay": pa.array([0.0, 1.0]),
        "r_az": pa.array([0.0, 0.0]),
        "r_angle": pa.array([1.5, 3.0]),
    })

    lens = DeriveLens("data", output_entity="/rot").to_rotation_axis_angle("ax", "ay", "az", "angle")

    chunks = (
        ParquetReader(path, index_columns=[IndexColumn.sequence("frame_index")])
        .stream()
        .lenses([lens], content="/r", output_mode="drop_unmatched")
        .to_chunks()
    )
    rot = _by_entity(chunks, "/rot").to_record_batch().column("Transform3D:rotation_axis_angle")

    assert rot.type.value_type == rr.components.RotationAxisAngle.arrow_type()
    assert rot.to_pylist() == [
        [{"axis": [1.0, 0.0, 0.0], "angle": 1.5}],
        [{"axis": [0.0, 1.0, 0.0], "angle": 3.0}],
    ]


def test_to_scalars(parquet_writer: ParquetWriter) -> None:
    """`to_scalars` maps struct fields to a multi-instance `Scalars:scalars` column (one series each)."""
    # Prefix `obs` → entity `/obs`, struct `data{vx, vy, vz}`.
    path = parquet_writer({
        "frame_index": pa.array([0, 1], pa.int64()),
        "obs_vx": pa.array([1.0, 2.0]),
        "obs_vy": pa.array([3.0, 4.0]),
        "obs_vz": pa.array([5.0, 6.0]),
    })

    lens = DeriveLens("data", output_entity="/obs").to_scalars("vx", "vy", "vz")

    chunks = (
        ParquetReader(path, index_columns=[IndexColumn.sequence("frame_index")])
        .stream()
        .lenses([lens], content="/obs", output_mode="drop_unmatched")
        .to_chunks()
    )
    scalars = _by_entity(chunks, "/obs").to_record_batch().column("Scalars:scalars")

    # Plain `List<f64>` with one instance (series) per field — *not* a nested fixed-size list.
    assert scalars.type.value_type == rr.components.Scalar.arrow_type()
    assert scalars.to_pylist() == [[1.0, 3.0, 5.0], [2.0, 4.0, 6.0]]


def test_to_scalars_single_field_is_plain_scalar(parquet_writer: ParquetWriter) -> None:
    """A single field is read as a plain scalar, not packed into a 1-element fixed-size list."""
    # Prefix `obs` → entity `/obs`, struct `data{vx, vy}`.
    path = parquet_writer({
        "frame_index": pa.array([0, 1], pa.int64()),
        "obs_vx": pa.array([1.0, 2.0]),
        "obs_vy": pa.array([3.0, 4.0]),
    })

    lens = DeriveLens("data", output_entity="/obs").to_scalars("vx")

    chunks = (
        ParquetReader(path, index_columns=[IndexColumn.sequence("frame_index")])
        .stream()
        .lenses([lens], content="/obs", output_mode="drop_unmatched")
        .to_chunks()
    )
    scalars = _by_entity(chunks, "/obs").to_record_batch().column("Scalars:scalars")

    # Plain scalar per row — the canonical Scalar datatype — and crucially *not* a fixed-size list.
    assert scalars.type.value_type == rr.components.Scalar.arrow_type()
    assert scalars.to_pylist() == [[1.0], [2.0]]


def test_named_scalar_series_via_lenses(parquet_writer: ParquetWriter) -> None:
    """
    End-to-end: map a timeseries to multi-value `Scalars` and co-locate static per-series names.

    `to_scalars` only produces the scalar values; `SeriesLines:names` is static metadata that must be
    injected by hand. We build that static chunk with `Chunk.from_columns(..., indexes=[])` and merge
    it into the reader stream, so both live at the same entity.
    """
    path = parquet_writer({
        "t": pa.array([0, 1, 2], pa.int64()),
        "obs_vx": pa.array([1.0, 2.0, 3.0]),
        "obs_vy": pa.array([4.0, 5.0, 6.0]),
        "obs_vz": pa.array([7.0, 8.0, 9.0]),
    })

    lens = DeriveLens("data", output_entity="/obs").to_scalars("vx", "vy", "vz")
    reader_stream = (
        ParquetReader(path, index_columns=[IndexColumn.sequence("t")])
        .stream()
        .lenses([lens], content="/obs", output_mode="drop_unmatched")
    )

    # Static names: empty `indexes` ⇒ static chunk; partition all 3 names into a single row.
    names_chunk = Chunk.from_columns(
        "/obs",
        indexes=[],
        columns=rr.SeriesLines.columns(names=["vx", "vy", "vz"]).partition(lengths=[3]),
    )

    store = LazyChunkStream.merge(reader_stream, LazyChunkStream.from_iter([names_chunk])).collect()

    obs_chunks = [c for c in store.stream().to_chunks() if c.entity_path == "/obs"]
    temporal = [c for c in obs_chunks if not c.is_static]
    static = [c for c in obs_chunks if c.is_static]

    assert len(temporal) == 1
    assert len(static) == 1

    scalars = temporal[0].to_record_batch().column("Scalars:scalars")
    assert scalars.to_pylist() == [[1.0, 4.0, 7.0], [2.0, 5.0, 8.0], [3.0, 6.0, 9.0]]

    names = static[0].to_record_batch().column("SeriesLines:names")
    assert names.to_pylist() == [["vx", "vy", "vz"]]


# ---------------------------------------------------------------------------
# StreamingReader protocol conformance
# ---------------------------------------------------------------------------


def test_streaming_reader_protocol(parquet_writer: ParquetWriter) -> None:
    path = parquet_writer({"x": pa.array([1.0])})
    assert isinstance(ParquetReader(path), StreamingReader)

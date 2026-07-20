"""Tests for `Chunk.from_record_batch` and `Chunk.from_dataframe`."""

from __future__ import annotations

from typing import TYPE_CHECKING

import pyarrow as pa
import pytest
import rerun as rr
from rerun import AUTO_INDEX
from rerun.experimental import Chunk, RrdReader

if TYPE_CHECKING:
    from pathlib import Path

# A simple list-of-floats component column, two rows.
_VALUES = pa.array([[1.0], [2.0]], type=pa.list_(pa.float32()))

SORBET_INDEX_NAME = b"rerun:index_name"
SORBET_ENTITY_PATH = b"rerun:entity_path"
RERUN_KIND = b"rerun:kind"


# ---------------------------------------------------------------------------
# Round-trip / identity
# ---------------------------------------------------------------------------


def test_round_trip_preserves_id() -> None:
    """A fully-annotated chunk batch round-trips to a single chunk, preserving the chunk id."""
    original = Chunk.from_columns(
        "/robots/arm",
        indexes=[rr.TimeColumn("frame", sequence=[0, 1, 2])],
        columns=rr.Points3D.columns(positions=[[1, 2, 3], [4, 5, 6], [7, 8, 9]]),
    )
    rb = original.to_record_batch()
    chunks = Chunk.from_record_batch(rb)
    assert len(chunks) == 1
    [restored] = chunks
    assert restored.id == original.id
    assert restored.entity_path == original.entity_path
    assert restored.num_rows == original.num_rows
    assert restored.num_columns == original.num_columns
    assert restored.is_static == original.is_static
    assert sorted(restored.timeline_names) == sorted(original.timeline_names)
    # The contents (data and schema metadata) round-trip identically.
    assert restored.to_record_batch().equals(rb, check_metadata=True)


def test_round_trip_from_rrd(test_rrd_path: Path) -> None:
    """to_record_batch() -> from_record_batch() round-trips real chunks read from an RRD."""
    chunks = RrdReader(test_rrd_path).stream().to_chunks()
    assert len(chunks) > 0

    for original in chunks:
        rb = original.to_record_batch()
        # A fully-annotated chunk batch round-trips to a single chunk, preserving identity.
        [restored] = Chunk.from_record_batch(rb)
        assert restored.id == original.id
        assert restored.entity_path == original.entity_path
        assert restored.num_rows == original.num_rows
        assert restored.num_columns == original.num_columns
        assert restored.is_static == original.is_static
        assert sorted(restored.timeline_names) == sorted(original.timeline_names)


# ---------------------------------------------------------------------------
# `index=` promotion
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    "index_type",
    [
        pa.int64(),
        pa.timestamp("ns"),
        pa.duration("ns"),
    ],
)
def test_index_promotion_time_types(index_type: pa.DataType) -> None:
    """`index=<column>` promotes the named column for each supported time dtype."""
    index = pa.array([0, 1], type=index_type)
    schema = pa.schema([
        pa.field("t", index.type),
        pa.field("/e:c", _VALUES.type, metadata={SORBET_ENTITY_PATH: b"/e"}),
    ])
    rb = pa.RecordBatch.from_arrays([index, _VALUES], schema=schema)
    [chunk] = Chunk.from_record_batch(rb, index="t")
    assert chunk.entity_path == "/e"
    assert chunk.timeline_names == ["t"]
    assert not chunk.is_static


def test_metadata_driven_temporal() -> None:
    """A batch tagged with `kind=index` (no explicit `index=`) is interpreted as temporal."""
    index = pa.array([0, 1], type=pa.int64())
    schema = pa.schema([
        pa.field("frame", index.type, metadata={RERUN_KIND: b"index"}),
        pa.field("/e:c", _VALUES.type, metadata={SORBET_ENTITY_PATH: b"/e"}),
    ])
    [chunk] = Chunk.from_record_batch(pa.RecordBatch.from_arrays([index, _VALUES], schema=schema))
    assert chunk.timeline_names == ["frame"]


def test_index_name_only_temporal() -> None:
    """An `index_name`-only column (no `rerun:kind`) is still promoted under AUTO."""
    index = pa.array([0, 1], type=pa.int64())
    schema = pa.schema([
        pa.field("frame", index.type, metadata={SORBET_INDEX_NAME: b"frame"}),
        pa.field("/e:c", _VALUES.type, metadata={SORBET_ENTITY_PATH: b"/e"}),
    ])
    [chunk] = Chunk.from_record_batch(pa.RecordBatch.from_arrays([index, _VALUES], schema=schema))
    assert chunk.timeline_names == ["frame"]


# ---------------------------------------------------------------------------
# Entity grouping / name convention
# ---------------------------------------------------------------------------


def test_multi_entity_split_preserves_order() -> None:
    """Component columns on different entities split into one chunk each, in first-seen order."""
    index = pa.array([0, 1], type=pa.int64())
    schema = pa.schema([
        pa.field("frame", index.type, metadata={RERUN_KIND: b"index"}),
        pa.field("/b:c", _VALUES.type, metadata={SORBET_ENTITY_PATH: b"/b"}),
        pa.field("/a:c", _VALUES.type, metadata={SORBET_ENTITY_PATH: b"/a"}),
    ])
    rb = pa.RecordBatch.from_arrays([index, _VALUES, _VALUES], schema=schema)
    chunks = Chunk.from_record_batch(rb)
    assert [c.entity_path for c in chunks] == ["/b", "/a"]


@pytest.mark.parametrize(
    ("name", "expected_entity"),
    [
        ("/e:c", "/e"),
        ("/e:Arch:c", "/e"),
        ("foo:bar", "/"),  # no leading slash → root
        ("property:foo", "/"),  # not recognized → root
    ],
)
def test_name_convention(name: str, expected_entity: str) -> None:
    """The column-name convention requires a leading `/`; otherwise the column lands on root."""
    index = pa.array([0, 1], type=pa.int64())
    schema = pa.schema([
        pa.field("frame", index.type, metadata={RERUN_KIND: b"index"}),
        pa.field(name, _VALUES.type),
    ])
    [chunk] = Chunk.from_record_batch(pa.RecordBatch.from_arrays([index, _VALUES], schema=schema))
    assert chunk.entity_path == expected_entity


def test_entity_path_argument() -> None:
    """`entity_path=` is the default for un-located component columns."""
    index = pa.array([0, 1], type=pa.int64())
    schema = pa.schema([
        pa.field("frame", index.type, metadata={RERUN_KIND: b"index"}),
        pa.field("bare", _VALUES.type),
    ])
    [chunk] = Chunk.from_record_batch(pa.RecordBatch.from_arrays([index, _VALUES], schema=schema), entity_path="/world")
    assert chunk.entity_path == "/world"


def test_plain_array_is_list_wrapped() -> None:
    """A plain (non-list) component array is wrapped as single-element lists."""
    index = pa.array([0, 1], type=pa.int64())
    plain = pa.array([1.0, 2.0], type=pa.float32())
    schema = pa.schema([
        pa.field("frame", index.type, metadata={RERUN_KIND: b"index"}),
        pa.field("/e:c", plain.type, metadata={SORBET_ENTITY_PATH: b"/e"}),
    ])
    [chunk] = Chunk.from_record_batch(pa.RecordBatch.from_arrays([index, plain], schema=schema))
    batch = chunk.to_record_batch()
    [component_field] = [f for f in batch.schema if f.metadata and f.metadata.get(RERUN_KIND) == b"data"]
    assert pa.types.is_list(component_field.type)


# ---------------------------------------------------------------------------
# Static
# ---------------------------------------------------------------------------


def test_static_single_row() -> None:
    """`index=None` with a single row produces a static chunk."""
    one = pa.array([[1.0]], type=pa.list_(pa.float32()))
    schema = pa.schema([pa.field("/e:c", one.type, metadata={SORBET_ENTITY_PATH: b"/e"})])
    [chunk] = Chunk.from_record_batch(pa.RecordBatch.from_arrays([one], schema=schema), index=None)
    assert chunk.is_static
    assert chunk.timeline_names == []


def test_static_with_index_metadata_is_contradiction() -> None:
    """`index=None` plus index metadata is a contradiction."""
    index = pa.array([0, 1], type=pa.int64())
    schema = pa.schema([
        pa.field("frame", index.type, metadata={RERUN_KIND: b"index"}),
        pa.field("/e:c", _VALUES.type, metadata={SORBET_ENTITY_PATH: b"/e"}),
    ])
    with pytest.raises(ValueError):
        Chunk.from_record_batch(pa.RecordBatch.from_arrays([index, _VALUES], schema=schema), index=None)


# ---------------------------------------------------------------------------
# Error cases (all `ValueError`)
# ---------------------------------------------------------------------------


def test_auto_no_index_raises() -> None:
    schema = pa.schema([pa.field("/e:c", _VALUES.type, metadata={SORBET_ENTITY_PATH: b"/e"})])
    with pytest.raises(ValueError):
        Chunk.from_record_batch(pa.RecordBatch.from_arrays([_VALUES], schema=schema))


def test_rejects_plain_batch() -> None:
    """A batch with no Rerun metadata at all is ambiguous under AUTO → ValueError."""
    plain_batch = pa.record_batch({"x": [1, 2, 3]})
    with pytest.raises(ValueError):
        Chunk.from_record_batch(plain_batch)


def test_null_in_index_raises() -> None:
    index = pa.array([0, None], type=pa.int64())
    schema = pa.schema([
        pa.field("frame", index.type, metadata={RERUN_KIND: b"index"}),
        pa.field("/e:c", _VALUES.type, metadata={SORBET_ENTITY_PATH: b"/e"}),
    ])
    with pytest.raises(ValueError):
        Chunk.from_record_batch(pa.RecordBatch.from_arrays([index, _VALUES], schema=schema))


@pytest.mark.parametrize(
    "bad_type",
    [
        pa.timestamp("us"),
        pa.duration("ms"),
        pa.time64("ns"),
    ],
)
def test_bad_time_dtype_raises(bad_type: pa.DataType) -> None:
    index = pa.array([0, 1], type=bad_type)
    schema = pa.schema([
        pa.field("t", index.type),
        pa.field("/e:c", _VALUES.type, metadata={SORBET_ENTITY_PATH: b"/e"}),
    ])
    with pytest.raises(ValueError):
        Chunk.from_record_batch(pa.RecordBatch.from_arrays([index, _VALUES], schema=schema), index="t")


def test_missing_named_index_raises() -> None:
    schema = pa.schema([pa.field("/e:c", _VALUES.type, metadata={SORBET_ENTITY_PATH: b"/e"})])
    with pytest.raises(ValueError):
        Chunk.from_record_batch(pa.RecordBatch.from_arrays([_VALUES], schema=schema), index="nope")


def test_no_component_columns_raises() -> None:
    index = pa.array([0, 1], type=pa.int64())
    schema = pa.schema([pa.field("frame", index.type, metadata={RERUN_KIND: b"index"})])
    with pytest.raises(ValueError):
        Chunk.from_record_batch(pa.RecordBatch.from_arrays([index], schema=schema))


# ---------------------------------------------------------------------------
# `from_dataframe`
# ---------------------------------------------------------------------------


def _temporal_batch() -> pa.RecordBatch:
    index = pa.array([0, 1], type=pa.int64())
    schema = pa.schema([
        pa.field("frame", index.type, metadata={RERUN_KIND: b"index"}),
        pa.field("/e:c", _VALUES.type, metadata={SORBET_ENTITY_PATH: b"/e"}),
    ])
    return pa.RecordBatch.from_arrays([index, _VALUES], schema=schema)


def test_from_dataframe_table() -> None:
    table = pa.Table.from_batches([_temporal_batch(), _temporal_batch()])
    chunks = list(Chunk.from_dataframe(table))
    assert len(chunks) == 2
    assert all(c.entity_path == "/e" for c in chunks)


def test_from_dataframe_reader() -> None:
    table = pa.Table.from_batches([_temporal_batch()])
    chunks = list(Chunk.from_dataframe(table.to_reader()))
    assert len(chunks) == 1


def test_from_dataframe_datafusion() -> None:
    """A `datafusion.DataFrame` is accepted via the soft dependency."""
    datafusion = pytest.importorskip("datafusion")

    table = pa.Table.from_batches([_temporal_batch()])
    ctx = datafusion.SessionContext()
    df = ctx.from_arrow(table)
    chunks = list(Chunk.from_dataframe(df))
    assert len(chunks) == 1
    assert chunks[0].entity_path == "/e"
    assert chunks[0].timeline_names == ["frame"]


def test_from_dataframe_validates_input_eagerly() -> None:
    """The input type is validated eagerly (not deferred to first iteration)."""
    with pytest.raises(TypeError):
        Chunk.from_dataframe("not a dataframe")  # type: ignore[arg-type]


def test_from_dataframe_record_batch() -> None:
    """A single `RecordBatch` is accepted (it implements the Arrow C stream interface)."""
    chunks = list(Chunk.from_dataframe(_temporal_batch()))
    assert len(chunks) == 1
    assert chunks[0].entity_path == "/e"


@pytest.mark.parametrize("bad", [123, b"bytes", object(), "not a dataframe"])
def test_from_dataframe_bad_input(bad: object) -> None:
    # Objects that are neither pyarrow Table/RecordBatchReader nor Arrow-C-stream sources are rejected.
    with pytest.raises(TypeError):
        Chunk.from_dataframe(bad)  # type: ignore[arg-type]


def test_auto_index_sentinel_is_default() -> None:
    """The default `index` is the `AUTO_INDEX` sentinel."""
    import inspect

    sig = inspect.signature(Chunk.from_record_batch)
    assert sig.parameters["index"].default is AUTO_INDEX

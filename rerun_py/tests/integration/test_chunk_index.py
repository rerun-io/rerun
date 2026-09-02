"""Tests for the private `ChunkIndex` observability surface (`_chunk_index()`)."""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest
import rerun as rr
from datafusion import col, lit
from rerun.chunk import ChunkStore, RrdReader

if TYPE_CHECKING:
    from pathlib import Path

    from rerun.experimental._chunk_index import ChunkIndex

CORE_COLUMNS = [
    "chunk_id",
    "chunk_is_static",
    "chunk_num_rows",
    "chunk_entity_path",
    "chunk_byte_offset",
    "chunk_byte_size",
    "chunk_byte_size_uncompressed",
]


@pytest.fixture(scope="session")
def chunk_index_rrd_path(tmp_path_factory: pytest.TempPathFactory) -> Path:
    """Four temporal chunks on `/a`, one on `/b`, one static chunk on `/c`."""
    rrd_path = tmp_path_factory.mktemp("chunk_index") / "test.rrd"

    with rr.RecordingStream("rerun_example_chunk_index", recording_id="chunk-index-rec-id") as rec:
        rec.save(rrd_path)

        for i in range(4):
            rec.send_columns(
                "/a",
                indexes=[rr.TimeColumn("tick", sequence=list(range(i * 10, i * 10 + 10)))],
                columns=rr.Scalars.columns(scalars=[float(j) for j in range(10)]),
            )
        rec.send_columns(
            "/b",
            indexes=[rr.TimeColumn("tick", sequence=[0])],
            columns=rr.Scalars.columns(scalars=[1.0]),
        )
        rec.send_columns("/c", indexes=[], columns=rr.TextLog.columns(text=["static"]))

    return rrd_path


def test_lazy_store_chunk_index(chunk_index_rrd_path: Path) -> None:
    """The `LazyStore` index is a footer read: correct shape, no chunk loads."""
    store = RrdReader(chunk_index_rrd_path).store()
    index = store._chunk_index()

    assert len(index) == len(store)
    assert index.store_id
    assert index.num_columns == index.to_arrow().num_columns

    missing = set(CORE_COLUMNS) - set(index.to_arrow().schema.names)
    assert not missing, f"missing per-chunk columns: {missing}"

    assert store._chunks_loaded == 0, "building the index must not load chunk data"


def test_chunk_index_df(chunk_index_rrd_path: Path) -> None:
    """`df()` drops the index into DataFusion, verbatim."""
    index = RrdReader(chunk_index_rrd_path).store()._chunk_index()
    df = index.df()

    assert df.count() == len(index)
    assert df.filter(col("chunk_entity_path") == lit("/a")).count() == 4
    assert df.filter(col("chunk_entity_path") == lit("/b")).count() == 1
    assert df.filter(col("chunk_is_static")).count() >= 1  # /c, plus recording properties


def test_chunk_store_chunk_index(chunk_index_rrd_path: Path) -> None:
    """The `ChunkStore` path builds an in-memory index with heap sizes."""
    store = ChunkStore.from_chunks(list(RrdReader(chunk_index_rrd_path).stream()))
    index = store._chunk_index()

    assert len(index) == len(store)

    sizes = index.to_arrow().column("chunk_byte_size_uncompressed").to_pylist()
    assert all(size > 0 for size in sizes)


def test_optimizer_round_trip(chunk_index_rrd_path: Path, tmp_path: Path) -> None:
    """The validation workflow: optimize, write, and compare the two files' indexes."""
    input_index = RrdReader(chunk_index_rrd_path).store()._chunk_index()

    out_path = tmp_path / "optimized.rrd"
    RrdReader(chunk_index_rrd_path).store()._optimized_stream().write_rrd(
        str(out_path),
        application_id="rerun_example_chunk_index",
        recording_id="chunk-index-optimized",
    )
    output_index = RrdReader(out_path).store()._chunk_index()

    # `/a`'s four small chunks merge into one; everything else passes through.
    assert len(output_index) < len(input_index)

    def total_rows(index: ChunkIndex) -> int:
        return sum(index.to_arrow().column("chunk_num_rows").to_pylist())

    assert total_rows(output_index) == total_rows(input_index)

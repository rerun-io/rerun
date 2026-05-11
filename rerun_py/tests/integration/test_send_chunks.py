"""Tests for rerun.experimental.send_chunks."""

from __future__ import annotations

from typing import TYPE_CHECKING, Protocol

import pytest
import rerun as rr
from rerun.experimental import Chunk, RrdReader

if TYPE_CHECKING:
    from collections.abc import Iterable, Iterator
    from pathlib import Path

    from rerun.experimental import ChunkStore, LazyChunkStream, LazyStore


class SendChunksAndRead(Protocol):
    """Send `chunks` to a fresh recording and return a reader for the result."""

    def __call__(
        self,
        chunks: Chunk | LazyChunkStream | LazyStore | ChunkStore | Iterable[Chunk],
    ) -> RrdReader: ...


def _make_chunk(entity_path: str, value: int) -> Chunk:
    return Chunk.from_columns(
        entity_path,
        indexes=[rr.TimeColumn("frame", sequence=[value])],
        columns=rr.Scalars.columns(scalars=[float(value)]),
    )


@pytest.fixture
def send_chunks_and_read(tmp_path: Path) -> SendChunksAndRead:
    """Fixture: send chunks into a fresh dest recording (one per call) and return a reader."""
    counter = 0

    def _impl(
        chunks: Chunk | LazyChunkStream | LazyStore | ChunkStore | Iterable[Chunk],
    ) -> RrdReader:
        nonlocal counter
        counter += 1
        out_path = tmp_path / f"out_{counter}.rrd"
        with rr.RecordingStream(
            "rerun_example_dest_app",
            recording_id="dest_rec",
            send_properties=False,
        ) as rec:
            rec.save(out_path)
            rec.send_chunks(chunks)
            rec.disconnect()
        return RrdReader(out_path)

    return _impl


# ---------------------------------------------------------------------------
# Single chunk
# ---------------------------------------------------------------------------


def test_send_single_chunk(send_chunks_and_read: SendChunksAndRead) -> None:
    chunk = _make_chunk("/single", 0)

    reader = send_chunks_and_read(chunk)

    paths = set(reader.store().schema().entity_paths())
    assert paths == {"/single"}


# ---------------------------------------------------------------------------
# Iterables
# ---------------------------------------------------------------------------


def test_send_iterable_chunks(send_chunks_and_read: SendChunksAndRead) -> None:
    chunks = [_make_chunk("/a", 0), _make_chunk("/b", 1)]

    reader = send_chunks_and_read(chunks)

    paths = set(reader.store().schema().entity_paths())
    assert paths == {"/a", "/b"}


def test_send_generator(send_chunks_and_read: SendChunksAndRead) -> None:
    def gen() -> Iterator[Chunk]:
        yield _make_chunk("/g0", 0)
        yield _make_chunk("/g1", 1)

    reader = send_chunks_and_read(gen())

    paths = set(reader.store().schema().entity_paths())
    assert paths == {"/g0", "/g1"}


# ---------------------------------------------------------------------------
# LazyChunkStream / LazyStore / ChunkStore
# ---------------------------------------------------------------------------


def test_send_lazy_chunk_stream(send_chunks_and_read: SendChunksAndRead, test_rrd_path: Path) -> None:
    src_paths = set(RrdReader(test_rrd_path).store().schema().entity_paths())

    stream = RrdReader(test_rrd_path).stream()
    reader = send_chunks_and_read(stream)

    dest_paths = set(reader.store().schema().entity_paths())
    assert dest_paths == src_paths


def test_send_lazy_chunk_stream_filtered(send_chunks_and_read: SendChunksAndRead, test_rrd_path: Path) -> None:
    stream = RrdReader(test_rrd_path).stream().filter(content="/robots/**")
    reader = send_chunks_and_read(stream)

    dest_paths = set(reader.store().schema().entity_paths())
    assert dest_paths == {"/robots/arm"}


def test_send_lazy_store(send_chunks_and_read: SendChunksAndRead, test_rrd_path: Path) -> None:
    via_store_reader = send_chunks_and_read(RrdReader(test_rrd_path).store())
    via_stream_reader = send_chunks_and_read(RrdReader(test_rrd_path).store().stream())

    via_store_paths = set(via_store_reader.store().schema().entity_paths())
    via_stream_paths = set(via_stream_reader.store().schema().entity_paths())
    assert via_store_paths == via_stream_paths


def test_send_chunk_store(send_chunks_and_read: SendChunksAndRead, test_rrd_path: Path) -> None:
    via_store_reader = send_chunks_and_read(RrdReader(test_rrd_path).stream().collect())
    via_stream_reader = send_chunks_and_read(RrdReader(test_rrd_path).stream().collect().stream())

    via_store_paths = set(via_store_reader.store().schema().entity_paths())
    via_stream_paths = set(via_stream_reader.store().schema().entity_paths())
    assert via_store_paths == via_stream_paths


# ---------------------------------------------------------------------------
# Edge cases
# ---------------------------------------------------------------------------


def test_send_chunks_iterable_type_error(tmp_path: Path) -> None:
    """Non-Chunk items in an iterable raise TypeError when drained."""
    out = tmp_path / "out.rrd"
    with rr.RecordingStream("rerun_example_dest_app", recording_id="dest_rec") as rec:
        rec.save(out)
        with pytest.raises(TypeError, match="Chunk"):
            rec.send_chunks(["not a chunk"])  # type: ignore[list-item]


def test_send_chunks_consumed_lazy_stream(tmp_path: Path, test_rrd_path: Path) -> None:
    """A LazyChunkStream consumed by a builder cannot be re-sent."""
    stream = RrdReader(test_rrd_path).stream()
    stream.filter(content="/robots/**")  # consumes `stream`

    out = tmp_path / "out.rrd"
    with rr.RecordingStream("rerun_example_dest_app", recording_id="dest_rec") as rec:
        rec.save(out)
        with pytest.raises(ValueError):
            rec.send_chunks(stream)

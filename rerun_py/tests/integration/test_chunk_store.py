"""Integration tests for ChunkStore."""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest
import rerun as rr
from rerun.experimental import (
    ChunkStore,
    LazyChunkStream,
    RrdReader,
)

from .conftest import TEST_APP_ID as APP_ID, TEST_RECORDING_ID as RECORDING_ID

if TYPE_CHECKING:
    from pathlib import Path

    from syrupy import SnapshotAssertion


@pytest.fixture(scope="session")
def fragmented_rrd_path(tmp_path_factory: pytest.TempPathFactory) -> Path:
    """RRD with many tiny single-row chunks, ideal for compaction testing."""

    rrd_path = tmp_path_factory.mktemp("compact") / "fragmented.rrd"

    with rr.RecordingStream("rerun_example_compact_test", recording_id="compact-test-id") as rec:
        rec.save(rrd_path)

        # 20 individual send_columns calls -> 20 separate chunks for the same entity
        for i in range(20):
            rec.send_columns(
                "/sensor",
                indexes=[rr.TimeColumn("frame", sequence=[i])],
                columns=rr.Scalars.columns(scalars=[float(i)]),
            )

    return rrd_path


# ---------------------------------------------------------------------------
# ChunkStore basics
# ---------------------------------------------------------------------------


def test_store_from_rrd_reader(test_rrd_path: Path) -> None:
    """RrdReader.store() returns a ChunkStore."""
    store = RrdReader(test_rrd_path).store()
    assert isinstance(store, ChunkStore)


def test_repr(test_rrd_path: Path) -> None:
    store = RrdReader(test_rrd_path).store()
    assert "ChunkStore" in repr(store)


# ---------------------------------------------------------------------------
# ChunkStore.schema()
# ---------------------------------------------------------------------------


def test_schema(test_rrd_path: Path, snapshot: SnapshotAssertion) -> None:
    """schema() returns a Schema matching the stored data."""
    store = RrdReader(test_rrd_path).store()
    assert repr(store.schema()) == snapshot


def test_schema_entity_paths(test_rrd_path: Path) -> None:
    store = RrdReader(test_rrd_path).store()
    paths = store.schema().entity_paths()
    assert "/robots/arm" in paths
    assert "/cameras/front" in paths
    assert "/config" in paths


# ---------------------------------------------------------------------------
# ChunkStore.stream()
# ---------------------------------------------------------------------------


def test_stream_returns_lazy_chunk_stream(test_rrd_path: Path) -> None:
    store = RrdReader(test_rrd_path).store()
    assert isinstance(store.stream(), LazyChunkStream)


def test_stream_is_repeatable(test_rrd_path: Path) -> None:
    """stream() can be called multiple times; each produces the same schema."""
    store = RrdReader(test_rrd_path).store()
    first = store.stream().collect()
    second = store.stream().collect()
    assert first.schema() == second.schema()


def test_stream_supports_pipeline_ops(test_rrd_path: Path) -> None:
    """Chunks from store().stream() work with filter/collect."""
    store = RrdReader(test_rrd_path).store()
    filtered = store.stream().filter(is_static=True).collect()
    assert filtered.schema().entity_paths() == ["/config"]


# ---------------------------------------------------------------------------
# Equivalence: store().stream() vs reader.stream()
# ---------------------------------------------------------------------------


def test_same_schema(test_rrd_path: Path) -> None:
    """store().stream().collect() and reader.stream().collect() produce the same schema."""
    reader = RrdReader(test_rrd_path)
    from_streaming = reader.stream().collect()
    from_store = reader.store().stream().collect()
    assert from_streaming.schema() == from_store.schema()


# ---------------------------------------------------------------------------
# ChunkStore.write_rrd()
# ---------------------------------------------------------------------------


def test_write_rrd_roundtrip(test_rrd_path: Path, tmp_path: Path) -> None:
    """write_rrd() -> RrdReader().store() preserves schema."""
    store1 = RrdReader(test_rrd_path).store()
    out = tmp_path / "roundtrip.rrd"
    store1.write_rrd(out, application_id=APP_ID, recording_id=RECORDING_ID)

    store2 = RrdReader(out).store()
    assert store1.schema() == store2.schema()


def test_write_rrd_metadata(test_rrd_path: Path, tmp_path: Path) -> None:
    """write_rrd() writes the provided application_id and recording_id."""
    store = RrdReader(test_rrd_path).store()
    out = tmp_path / "meta.rrd"
    store.write_rrd(out, application_id="my-app", recording_id="my-rec")

    reader = RrdReader(out)
    assert reader.application_id == "my-app"
    assert reader.recording_id == "my-rec"


# ---------------------------------------------------------------------------
# ChunkStore.compact()
# ---------------------------------------------------------------------------


def test_compact_reduces_chunks(fragmented_rrd_path: Path) -> None:
    """compact() merges small chunks into fewer, larger ones."""
    store = RrdReader(fragmented_rrd_path).store()
    before = len(store.stream().to_chunks())

    compacted = store.compact()
    after = len(compacted.stream().to_chunks())

    assert after < before


def test_compact_preserves_schema(fragmented_rrd_path: Path) -> None:
    """compact() preserves the schema."""
    store = RrdReader(fragmented_rrd_path).store()
    compacted = store.compact()
    assert store.schema() == compacted.schema()


def test_compact_preserves_row_count(fragmented_rrd_path: Path) -> None:
    """compact() preserves the total number of rows across all chunks."""
    store = RrdReader(fragmented_rrd_path).store()
    compacted = store.compact()

    original_rows = sum(c.num_rows for c in store.stream().to_chunks())
    compacted_rows = sum(c.num_rows for c in compacted.stream().to_chunks())
    assert compacted_rows == original_rows

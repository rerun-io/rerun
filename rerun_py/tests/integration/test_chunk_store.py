"""Integration tests for ChunkStore."""

from __future__ import annotations

from typing import TYPE_CHECKING

from rerun.experimental import (
    ChunkStore,
    LazyChunkStream,
    RrdLoader,
)

from .conftest import TEST_APP_ID as APP_ID, TEST_RECORDING_ID as RECORDING_ID

if TYPE_CHECKING:
    from pathlib import Path

    from syrupy import SnapshotAssertion


# ---------------------------------------------------------------------------
# ChunkStore basics
# ---------------------------------------------------------------------------


def test_store_from_rrd_loader(test_rrd_path: Path) -> None:
    """RrdLoader.store() returns a ChunkStore."""
    store = RrdLoader(test_rrd_path).store()
    assert isinstance(store, ChunkStore)


def test_repr(test_rrd_path: Path) -> None:
    store = RrdLoader(test_rrd_path).store()
    assert "ChunkStore" in repr(store)


# ---------------------------------------------------------------------------
# ChunkStore.schema()
# ---------------------------------------------------------------------------


def test_schema(test_rrd_path: Path, snapshot: SnapshotAssertion) -> None:
    """schema() returns a Schema matching the stored data."""
    store = RrdLoader(test_rrd_path).store()
    assert repr(store.schema()) == snapshot


def test_schema_entity_paths(test_rrd_path: Path) -> None:
    store = RrdLoader(test_rrd_path).store()
    paths = store.schema().entity_paths()
    assert "/robots/arm" in paths
    assert "/cameras/front" in paths
    assert "/config" in paths


# ---------------------------------------------------------------------------
# ChunkStore.stream()
# ---------------------------------------------------------------------------


def test_stream_returns_lazy_chunk_stream(test_rrd_path: Path) -> None:
    store = RrdLoader(test_rrd_path).store()
    assert isinstance(store.stream(), LazyChunkStream)


def test_stream_is_repeatable(test_rrd_path: Path) -> None:
    """stream() can be called multiple times; each produces the same schema."""
    store = RrdLoader(test_rrd_path).store()
    first = store.stream().collect()
    second = store.stream().collect()
    assert first.schema() == second.schema()


def test_stream_supports_pipeline_ops(test_rrd_path: Path) -> None:
    """Chunks from store().stream() work with filter/collect."""
    store = RrdLoader(test_rrd_path).store()
    filtered = store.stream().filter(is_static=True).collect()
    assert filtered.schema().entity_paths() == ["/config"]


# ---------------------------------------------------------------------------
# Equivalence: store().stream() vs loader.stream()
# ---------------------------------------------------------------------------


def test_same_schema(test_rrd_path: Path) -> None:
    """store().stream().collect() and loader.stream().collect() produce the same schema."""
    loader = RrdLoader(test_rrd_path)
    from_streaming = loader.stream().collect()
    from_store = loader.store().stream().collect()
    assert from_streaming.schema() == from_store.schema()


# ---------------------------------------------------------------------------
# ChunkStore.write_rrd()
# ---------------------------------------------------------------------------


def test_write_rrd_roundtrip(test_rrd_path: Path, tmp_path: Path) -> None:
    """write_rrd() -> RrdLoader().store() preserves schema."""
    store1 = RrdLoader(test_rrd_path).store()
    out = tmp_path / "roundtrip.rrd"
    store1.write_rrd(out, application_id=APP_ID, recording_id=RECORDING_ID)

    store2 = RrdLoader(out).store()
    assert store1.schema() == store2.schema()


def test_write_rrd_metadata(test_rrd_path: Path, tmp_path: Path) -> None:
    """write_rrd() writes the provided application_id and recording_id."""
    store = RrdLoader(test_rrd_path).store()
    out = tmp_path / "meta.rrd"
    store.write_rrd(out, application_id="my-app", recording_id="my-rec")

    loader = RrdLoader(out)
    assert loader.application_id == "my-app"
    assert loader.recording_id == "my-rec"

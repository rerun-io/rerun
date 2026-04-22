"""Integration tests for lazy ChunkStore loading."""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest
import rerun as rr
from rerun.experimental import (
    OptimizationSettings,
    RrdReader,
)

if TYPE_CHECKING:
    from pathlib import Path

LAZY_RRD_APPLICATION_ID = "rerun_example_lazy_test_app"
LAZY_RRD_RECORDING_ID = "lazy-rrd-rec-id"


@pytest.fixture(scope="session")
def lazy_rrd_path(tmp_path_factory: pytest.TempPathFactory) -> Path:
    """RRD with known structure, suitable for lazy loading tests."""

    rrd_path = tmp_path_factory.mktemp("lazy") / "test.rrd"

    with rr.RecordingStream(LAZY_RRD_APPLICATION_ID, recording_id=LAZY_RRD_RECORDING_ID) as rec:
        rec.save(rrd_path)

        for i in range(5):
            rec.send_columns(
                f"/entity_{i}",
                indexes=[rr.TimeColumn("frame", sequence=list(range(10)))],
                columns=rr.Scalars.columns(scalars=[float(j) for j in range(10)]),
            )

    return rrd_path


def test_lazy_store_has_schema(lazy_rrd_path: Path) -> None:
    """Lazy store should have a schema even before loading chunk data."""
    store = RrdReader(lazy_rrd_path).store()
    schema = store.schema()
    assert schema is not None


def test_lazy_store_stream_to_chunks(lazy_rrd_path: Path) -> None:
    """Lazy store's stream should produce the same chunks as direct streaming."""
    reader = RrdReader(lazy_rrd_path)

    store_chunks = reader.store().stream().to_chunks()
    stream_chunks = reader.stream().to_chunks()

    # Same number of chunks.
    assert len(store_chunks) == len(stream_chunks)

    # Same entity paths (as sorted sets — ordering may differ).
    store_entities = sorted({str(c.entity_path) for c in store_chunks})
    stream_entities = sorted({str(c.entity_path) for c in stream_chunks})
    assert store_entities == stream_entities


def test_lazy_store_roundtrip(lazy_rrd_path: Path, tmp_path: Path) -> None:
    """Write a lazily-loaded store to a new RRD and reload it."""
    store = RrdReader(lazy_rrd_path).store()
    original_chunks = store.stream().to_chunks()

    out_path = tmp_path / "roundtrip.rrd"
    store.stream().write_rrd(
        str(out_path),
        application_id="roundtrip_test",
        recording_id="roundtrip-id",
    )

    reloaded_chunks = RrdReader(str(out_path)).store().stream().to_chunks()
    assert len(reloaded_chunks) == len(original_chunks)


def test_lazy_store_filter(lazy_rrd_path: Path) -> None:
    """Filtering on a lazy store's stream should work."""
    store = RrdReader(lazy_rrd_path).store()
    filtered = store.stream().filter(content="/entity_0").to_chunks()

    assert len(filtered) > 0
    for chunk in filtered:
        assert str(chunk.entity_path) == "/entity_0"


def test_lazy_store_collect_optimize(lazy_rrd_path: Path) -> None:
    """Collecting a lazy store with optimization settings produces a materialized store."""
    store = RrdReader(lazy_rrd_path).store()
    optimized = store.stream().collect(optimize=OptimizationSettings())

    chunks = optimized.stream().to_chunks()
    assert len(chunks) > 0


def test_multiple_store_calls(lazy_rrd_path: Path) -> None:
    """Multiple .store() calls should return independent stores."""
    reader = RrdReader(lazy_rrd_path)
    store1 = reader.store()
    store2 = reader.store()

    chunks1 = store1.stream().to_chunks()
    chunks2 = store2.stream().to_chunks()

    assert len(chunks1) == len(chunks2)


def test_store_properties(lazy_rrd_path: Path) -> None:
    """Application and recording IDs should be accessible."""
    reader = RrdReader(lazy_rrd_path)
    assert reader.application_id == LAZY_RRD_APPLICATION_ID
    assert reader.recording_id == LAZY_RRD_RECORDING_ID

    # Store should also work.
    store = reader.store()
    assert store is not None

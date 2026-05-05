from __future__ import annotations

from typing import TYPE_CHECKING

import pytest
from rerun.catalog import NotFoundError
from rerun.experimental import LazyStore, RrdReader

if TYPE_CHECKING:
    from pathlib import Path

    from rerun.catalog import DatasetEntry

    from e2e_redap_tests.conftest import EntryFactory


@pytest.fixture(scope="module")
def first_segment_store(readonly_test_dataset: DatasetEntry) -> LazyStore:
    """The `LazyStore` for the first segment in [`readonly_test_dataset`][]."""
    segment_ids = readonly_test_dataset.segment_ids()
    assert len(segment_ids) > 0
    return readonly_test_dataset.segment_store(segment_ids[0])


@pytest.fixture
def single_segment_store(entry_factory: EntryFactory, resource_prefix: str) -> LazyStore:
    """A `LazyStore` over a freshly-registered dataset containing exactly one segment."""
    ds = entry_factory.create_dataset("single_segment")
    handle = ds.register([resource_prefix + "dataset/file1.rrd"])
    handle.wait(timeout_secs=50)
    segment_ids = ds.segment_ids()
    assert len(segment_ids) == 1
    return ds.segment_store(segment_ids[0])


def test_segment_store_basic(first_segment_store: LazyStore) -> None:
    assert isinstance(first_segment_store, LazyStore)
    assert len(first_segment_store) > 0
    paths = first_segment_store.schema().entity_paths()
    assert any(p.startswith("/obj") for p in paths), f"got {paths!r}"


def test_segment_store_summary_uses_manifest(first_segment_store: LazyStore) -> None:
    """`summary()` walks the manifest only — no chunk fetch."""
    summary = first_segment_store.summary()
    assert summary
    assert "rows=" in summary


def test_segment_store_stream_to_chunks(first_segment_store: LazyStore) -> None:
    chunks = first_segment_store.stream().to_chunks()
    assert len(chunks) > 0
    for chunk in chunks:
        assert chunk.num_rows > 0


def test_segment_store_write_rrd_roundtrip(single_segment_store: LazyStore, tmp_path: Path) -> None:
    """Round-trip a single segment through `write_rrd`: schema and chunk count are preserved."""
    out = tmp_path / "out.rrd"

    single_segment_store.stream().write_rrd(out, application_id="rerun_example_test", recording_id="rec")

    roundtripped = RrdReader(out).store()
    assert roundtripped.schema() == single_segment_store.schema()
    assert len(roundtripped) == len(single_segment_store)


def test_segment_store_unknown_segment_raises(readonly_test_dataset: DatasetEntry) -> None:
    """Unknown segment id surfaces synchronously at construction (eager manifest)."""
    with pytest.raises(NotFoundError, match=r"does-not-exist"):
        readonly_test_dataset.segment_store("does-not-exist")


def test_segment_store_compile_twice_works(first_segment_store: LazyStore) -> None:
    """Each `compile()` opens its own FetchChunks; same chunks both times."""
    stream = first_segment_store.stream()

    first = stream.to_chunks()
    second = stream.to_chunks()
    assert len(first) == len(second)
    assert {c.id for c in first} == {c.id for c in second}

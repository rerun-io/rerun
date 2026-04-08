"""Tests for rerun.experimental.LazyChunkStream and RrdLoader."""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest
from rerun.experimental import Chunk, LazyChunkStream, RrdLoader

from .conftest import TEST_APP_ID as APP_ID, TEST_RECORDING_ID as RECORDING_ID

if TYPE_CHECKING:
    from pathlib import Path


# ---------------------------------------------------------------------------
# RrdLoader basics
# ---------------------------------------------------------------------------


def test_rrd_loader_properties(test_rrd_path: Path) -> None:
    loader = RrdLoader(test_rrd_path)
    assert loader.application_id == APP_ID
    assert loader.recording_id == RECORDING_ID


def test_rrd_loader_file_not_found(tmp_path: Path) -> None:
    with pytest.raises(ValueError, match="not found"):
        RrdLoader(tmp_path / "nonexistent.rrd")


# ---------------------------------------------------------------------------
# to_chunks / iter
# ---------------------------------------------------------------------------


def test_to_chunks(test_rrd_path: Path) -> None:
    """to_chunks() returns Chunk objects with expected properties."""
    chunks = RrdLoader(test_rrd_path).stream().to_chunks()

    assert len(chunks) > 0
    for chunk in chunks:
        assert isinstance(chunk, Chunk)
        assert chunk.num_rows > 0
        chunk.format(redact=True)


def test_iter(test_rrd_path: Path) -> None:
    stream = RrdLoader(test_rrd_path).stream()
    collected = stream.to_chunks()

    stream2 = RrdLoader(test_rrd_path).stream()
    iterated = list(stream2)

    assert len(iterated) == len(collected)


# ---------------------------------------------------------------------------
# collect
# ---------------------------------------------------------------------------


def test_collect_returns_chunk_store(test_rrd_path: Path) -> None:
    """collect() returns a ChunkStore with correct entity paths."""
    store = RrdLoader(test_rrd_path).stream().collect()
    paths = store.schema().entity_paths()
    assert "/robots/arm" in paths
    assert "/cameras/front" in paths
    assert "/config" in paths


# ---------------------------------------------------------------------------
# Identity roundtrip
# ---------------------------------------------------------------------------


def test_identity_roundtrip(test_rrd_path: Path, tmp_path: Path) -> None:
    import subprocess

    loader = RrdLoader(test_rrd_path)
    out = tmp_path / "roundtrip.rrd"

    loader.stream().write_rrd(out, application_id=APP_ID, recording_id=RECORDING_ID)

    original = loader.store()
    roundtripped = RrdLoader(out).store()
    assert original.schema() == roundtripped.schema()

    # Strong check: `rerun rrd compare` verifies semantic equality of the data.
    process = subprocess.run(
        ["rerun", "rrd", "compare", "--unordered", str(test_rrd_path), str(out)],
        check=False,
        capture_output=True,
    )
    if process.returncode != 0:
        print(process.stdout.decode("utf-8"))
        print(process.stderr.decode("utf-8"))
    assert process.returncode == 0, f"RRD compare failed: {process.stderr.decode('utf-8')}"


# ---------------------------------------------------------------------------
# filter
# ---------------------------------------------------------------------------


def test_filter_content(test_rrd_path: Path) -> None:
    # Single string
    store = RrdLoader(test_rrd_path).stream().filter(content="/robots/**").collect()
    assert store.schema().entity_paths() == ["/robots/arm"]

    # List of strings
    store2 = RrdLoader(test_rrd_path).stream().filter(content=["/robots/**", "/cameras/**"]).collect()
    assert store2.schema().entity_paths() == ["/cameras/front", "/robots/arm"]


def test_filter_is_static(test_rrd_path: Path) -> None:
    static_store = RrdLoader(test_rrd_path).stream().filter(is_static=True).collect()
    assert static_store.schema().entity_paths() == ["/config"]

    temporal_store = RrdLoader(test_rrd_path).stream().filter(is_static=False).collect()
    assert "/config" not in temporal_store.schema().entity_paths()


def test_filter_has_timeline(test_rrd_path: Path) -> None:
    store = RrdLoader(test_rrd_path).stream().filter(has_timeline="other_timeline").collect()
    # Only /robots/arm has the other_timeline
    assert store.schema().entity_paths() == ["/robots/arm"]


def test_filter_component(test_rrd_path: Path) -> None:
    store = RrdLoader(test_rrd_path).stream().filter(components="Points3D:positions").collect()
    assert store.schema().entity_paths() == ["/robots/arm"]
    # colors column should be stripped — only positions remains
    arm_cols = store.schema().columns_for(entity_path="/robots/arm")
    assert len(arm_cols) == 1
    assert arm_cols[0].component == "Points3D:positions"


def test_component_slice_gets_new_chunk_id(test_rrd_path: Path) -> None:
    """Slicing by component must produce chunks with fresh IDs, not reuse the original."""
    original_ids = {c.id for c in RrdLoader(test_rrd_path).stream().to_chunks()}

    # filter keeps only the matching column -> sliced chunk
    filtered = RrdLoader(test_rrd_path).stream().filter(components="Points3D:positions").to_chunks()
    for chunk in filtered:
        assert chunk.id not in original_ids, "filter(components=...) must assign a new ChunkId"  # NOLINT

    # drop keeps the non-matching columns -> also a sliced chunk
    dropped = RrdLoader(test_rrd_path).stream().drop(components="Points3D:positions").to_chunks()
    for chunk in dropped:
        if chunk.entity_path == "/robots/arm":
            assert chunk.id not in original_ids, "drop(components=...) must assign a new ChunkId"  # NOLINT


def test_filter_multiple_components(test_rrd_path: Path) -> None:
    """filter(components=[A, B]) keeps both columns when present (OR semantics)."""
    store = RrdLoader(test_rrd_path).stream().filter(components=["Points3D:positions", "Points3D:colors"]).collect()
    assert store.schema().entity_paths() == ["/robots/arm"]
    arm_cols = store.schema().columns_for(entity_path="/robots/arm")
    assert len(arm_cols) == 2


def test_filter_multiple_components_partial(test_rrd_path: Path) -> None:
    """filter(components=[A, Z]) where Z doesn't exist: keep A only."""
    store = RrdLoader(test_rrd_path).stream().filter(components=["Points3D:positions", "Nonexistent:foo"]).collect()
    assert store.schema().entity_paths() == ["/robots/arm"]


def test_filter_multiple_components_none_present(test_rrd_path: Path) -> None:
    """filter(components=[Z1, Z2]) where neither exist: empty store."""
    store = RrdLoader(test_rrd_path).stream().filter(components=["Nonexistent:a", "Nonexistent:b"]).collect()
    assert store.schema().entity_paths() == []


def test_drop_multiple_components(test_rrd_path: Path) -> None:
    """drop(components=[A, B]) removes both columns."""
    store = RrdLoader(test_rrd_path).stream().drop(components=["Points3D:positions", "Points3D:colors"]).collect()
    # /robots/arm had only those two components, so it should be gone
    assert "/robots/arm" not in store.schema().entity_paths()


def test_split_multiple_components(test_rrd_path: Path) -> None:
    """split(components=[A, B]): matched gets A+B, complement gets rest."""
    stream = RrdLoader(test_rrd_path).stream()
    matched, complement = stream.split(components=["Points3D:positions", "Points3D:colors"])

    matched_store = matched.collect()
    complement_store = complement.collect()

    assert matched_store.schema().entity_paths() == ["/robots/arm"]
    assert len(complement_store.schema().entity_paths()) > 0


# ---------------------------------------------------------------------------
# drop
# ---------------------------------------------------------------------------


def test_drop(test_rrd_path: Path) -> None:
    store = RrdLoader(test_rrd_path).stream().drop(content="/robots/**").collect()
    paths = store.schema().entity_paths()
    assert "/robots/arm" not in paths
    assert "/cameras/front" in paths
    assert "/config" in paths


# ---------------------------------------------------------------------------
# split / merge
# ---------------------------------------------------------------------------


def test_split_merge_roundtrip(test_rrd_path: Path) -> None:
    original = RrdLoader(test_rrd_path).stream().collect()

    stream2 = RrdLoader(test_rrd_path).stream()
    static_branch, temporal_branch = stream2.split(is_static=True)
    merged = LazyChunkStream.merge(static_branch, temporal_branch).collect()

    assert original.schema() == merged.schema()


def test_split_drop_one_branch(test_rrd_path: Path) -> None:
    """Consuming only one branch of a split should not hang."""
    stream = RrdLoader(test_rrd_path).stream()
    matching, _non_matching = stream.split(content="/robots/**")

    store = matching.collect()
    assert store.schema().entity_paths() == ["/robots/arm"]


# ---------------------------------------------------------------------------
# from_iter
# ---------------------------------------------------------------------------


def test_from_iter(test_rrd_path: Path) -> None:
    original = RrdLoader(test_rrd_path).stream().to_chunks()

    roundtripped = LazyChunkStream.from_iter(original).collect()
    assert roundtripped.schema() == RrdLoader(test_rrd_path).store().schema()


# ---------------------------------------------------------------------------
# Composition
# ---------------------------------------------------------------------------


def test_chained_filters(test_rrd_path: Path) -> None:
    store = RrdLoader(test_rrd_path).stream().filter(is_static=False).filter(content="/robots/**").collect()
    assert store.schema().entity_paths() == ["/robots/arm"]


# ---------------------------------------------------------------------------
# Dangling split branch
# ---------------------------------------------------------------------------


def test_dangling_split_matched_only(test_rrd_path: Path, capfd: pytest.CaptureFixture[str]) -> None:
    """Using only the matched branch of a split should work (degenerated to filter) and warn."""
    stream = RrdLoader(test_rrd_path).stream()
    matched, _unmatched = stream.split(content="/robots/**")

    store = matched.collect()
    assert store.schema().entity_paths() == ["/robots/arm"]

    captured = capfd.readouterr()
    assert "only one branch" in captured.err.lower(), (
        f"Expected a warning about dangling split branch on stderr, got: {captured.err!r}"
    )


def test_dangling_split_unmatched_only(test_rrd_path: Path, capfd: pytest.CaptureFixture[str]) -> None:
    """Using only the unmatched branch of a split should work (degenerated to drop) and warn."""
    stream = RrdLoader(test_rrd_path).stream()
    _matched, unmatched = stream.split(content="/robots/**")

    store = unmatched.collect()
    assert "/robots/arm" not in store.schema().entity_paths()

    captured = capfd.readouterr()
    assert "only one branch" in captured.err.lower(), (
        f"Expected a warning about dangling split branch on stderr, got: {captured.err!r}"
    )


# ---------------------------------------------------------------------------
# Move semantics
# ---------------------------------------------------------------------------


def test_stream_consumed_after_filter(test_rrd_path: Path) -> None:
    """A stream consumed by filter() cannot be used again as a builder input."""
    stream = RrdLoader(test_rrd_path).stream()
    _filtered = stream.filter(is_static=True)

    with pytest.raises(ValueError, match="already been consumed"):
        stream.drop(is_static=False)


def test_stream_consumed_after_split(test_rrd_path: Path) -> None:
    """A stream consumed by split() cannot be used again as a builder input."""
    stream = RrdLoader(test_rrd_path).stream()
    _a, _b = stream.split(is_static=True)

    with pytest.raises(ValueError, match="already been consumed"):
        stream.filter(content="/foo/**")


def test_merge_same_stream_twice(test_rrd_path: Path) -> None:
    """Passing the same stream to merge twice is an error."""
    stream = RrdLoader(test_rrd_path).stream()
    a, _b = stream.split(is_static=True)

    with pytest.raises(ValueError, match="already been consumed"):
        LazyChunkStream.merge(a, a)


def test_merge_indirect_reuse(test_rrd_path: Path) -> None:
    """A stream used as split upstream and also passed directly to merge is an error."""
    stream = RrdLoader(test_rrd_path).stream()
    a, b = stream.split(is_static=True)
    _b1, b2 = b.split(content="/robots/**")

    # b was consumed by the second split, so passing it to merge should fail.
    with pytest.raises(ValueError, match="already been consumed"):
        LazyChunkStream.merge(b, b2, a)


def test_terminal_does_not_consume(test_rrd_path: Path) -> None:
    """Terminals (collect, write_rrd, iter) borrow without consuming."""
    stream = RrdLoader(test_rrd_path).stream()

    store1 = stream.collect()
    store2 = stream.collect()
    assert store1.schema() == store2.schema()

    chunks = list(stream)
    assert len(chunks) > 0

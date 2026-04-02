"""Tests for rerun.experimental.LazyChunkStream and RrdLoader."""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest
import rerun as rr
from rerun.experimental import Chunk, LazyChunkStream, RrdLoader

if TYPE_CHECKING:
    from pathlib import Path

APP_ID = "test_lazy_chunk_stream"
RECORDING_ID = "fixed-recording-id-for-tests"


@pytest.fixture(scope="session")
def test_rrd_path(tmp_path_factory: pytest.TempPathFactory) -> Path:
    """Session-scoped RRD with known entity paths, timelines, and component structure."""

    rrd_path = tmp_path_factory.mktemp("lazy_chunk_stream") / "test.rrd"

    with rr.RecordingStream(APP_ID, recording_id=RECORDING_ID) as rec:
        rec.save(rrd_path)

        # Temporal: two timelines, Points3D with positions + colors
        rec.send_columns(
            "/robots/arm",
            indexes=[
                rr.TimeColumn("my_index", sequence=[1, 2]),
                rr.TimeColumn("other_timeline", sequence=[10, 20]),
            ],
            columns=rr.Points3D.columns(
                positions=[[1, 2, 3], [4, 5, 6]],
                colors=[[255, 0, 0], [0, 255, 0]],
            ),
        )

        # Temporal: one timeline, TextLog
        rec.send_columns(
            "/cameras/front",
            indexes=[rr.TimeColumn("my_index", sequence=[1])],
            columns=rr.TextLog.columns(text=["frame_001"]),
        )

        # Static: no timelines, TextLog
        rec.send_columns(
            "/config",
            indexes=[],
            columns=rr.TextLog.columns(text=["v1"]),
        )

    return rrd_path


# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------


def entity_paths(chunks: list[Chunk]) -> set[str]:
    return {c.entity_path for c in chunks}


def non_property_chunks(chunks: list[Chunk]) -> list[Chunk]:
    """Filter out auto-generated __properties chunks."""
    return [c for c in chunks if not c.entity_path.startswith("/__")]


# ---------------------------------------------------------------------------
# RrdLoader basics
# ---------------------------------------------------------------------------


@pytest.mark.local_only
def test_rrd_loader_properties(test_rrd_path: Path) -> None:
    loader = RrdLoader(test_rrd_path)
    assert loader.application_id == APP_ID
    assert loader.recording_id == RECORDING_ID


@pytest.mark.local_only
def test_rrd_loader_file_not_found(tmp_path: Path) -> None:
    with pytest.raises(ValueError, match="not found"):
        RrdLoader(tmp_path / "nonexistent.rrd")


# ---------------------------------------------------------------------------
# Collect / iter
# ---------------------------------------------------------------------------


@pytest.mark.local_only
def test_collect(test_rrd_path: Path) -> None:
    chunks = non_property_chunks(RrdLoader(test_rrd_path).stream().collect())

    assert len(chunks) > 0
    paths = entity_paths(chunks)
    assert "/robots/arm" in paths
    assert "/cameras/front" in paths
    assert "/config" in paths

    for chunk in chunks:
        assert isinstance(chunk, Chunk)
        assert chunk.num_rows > 0
        # format with redact should not raise
        chunk.format(redact=True)


@pytest.mark.local_only
def test_iter(test_rrd_path: Path) -> None:
    stream = RrdLoader(test_rrd_path).stream()
    collected = non_property_chunks(stream.collect())

    stream2 = RrdLoader(test_rrd_path).stream()
    iterated = non_property_chunks(list(stream2))

    assert len(iterated) == len(collected)
    assert entity_paths(iterated) == entity_paths(collected)


# ---------------------------------------------------------------------------
# Identity roundtrip
# ---------------------------------------------------------------------------


@pytest.mark.local_only
def test_identity_roundtrip(test_rrd_path: Path, tmp_path: Path) -> None:
    import subprocess

    loader = RrdLoader(test_rrd_path)
    out = tmp_path / "roundtrip.rrd"

    loader.stream().write_rrd(out, application_id=APP_ID, recording_id=RECORDING_ID)

    # Sanity check: same entity paths and chunk count.
    original = non_property_chunks(RrdLoader(test_rrd_path).stream().collect())
    roundtripped = non_property_chunks(RrdLoader(out).stream().collect())
    assert len(roundtripped) == len(original)
    assert entity_paths(roundtripped) == entity_paths(original)

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


@pytest.mark.local_only
def test_filter_content(test_rrd_path: Path) -> None:
    stream = RrdLoader(test_rrd_path).stream()

    # Single string
    chunks = non_property_chunks(stream.filter(content="/robots/**").collect())
    assert entity_paths(chunks) == {"/robots/arm"}

    # List of strings
    stream2 = RrdLoader(test_rrd_path).stream()
    chunks2 = non_property_chunks(stream2.filter(content=["/robots/**", "/cameras/**"]).collect())
    assert entity_paths(chunks2) == {"/robots/arm", "/cameras/front"}


@pytest.mark.local_only
def test_filter_is_static(test_rrd_path: Path) -> None:
    stream = RrdLoader(test_rrd_path).stream()
    static_chunks = non_property_chunks(stream.filter(is_static=True).collect())
    assert all(c.is_static for c in static_chunks)
    assert entity_paths(static_chunks) == {"/config"}

    stream2 = RrdLoader(test_rrd_path).stream()
    temporal_chunks = non_property_chunks(stream2.filter(is_static=False).collect())
    assert all(not c.is_static for c in temporal_chunks)
    assert "/config" not in entity_paths(temporal_chunks)


@pytest.mark.local_only
def test_filter_has_timeline(test_rrd_path: Path) -> None:
    stream = RrdLoader(test_rrd_path).stream()
    chunks = non_property_chunks(stream.filter(has_timeline="other_timeline").collect())

    # Only /robots/arm has the other_timeline
    assert entity_paths(chunks) == {"/robots/arm"}
    for chunk in chunks:
        assert "other_timeline" in chunk.timeline_names


@pytest.mark.local_only
def test_filter_component(test_rrd_path: Path) -> None:
    stream = RrdLoader(test_rrd_path).stream()
    chunks = stream.filter(components="Points3D:positions").collect()

    # Only chunks that had Points3D:positions survive
    assert len(chunks) > 0
    for chunk in chunks:
        assert chunk.entity_path == "/robots/arm"
        # The chunk should have fewer columns than the unfiltered version
        # (colors column should be stripped)


@pytest.mark.local_only
def test_component_slice_gets_new_chunk_id(test_rrd_path: Path) -> None:
    """Slicing by component must produce chunks with fresh IDs, not reuse the original."""
    original_chunks = RrdLoader(test_rrd_path).stream().collect()
    original_ids = {c.id for c in original_chunks}

    # filter keeps only the matching column -> sliced chunk
    filtered = RrdLoader(test_rrd_path).stream().filter(components="Points3D:positions").collect()
    for chunk in filtered:
        assert chunk.id not in original_ids, "filter(components=...) must assign a new ChunkId"  # NOLINT

    # drop keeps the non-matching columns -> also a sliced chunk
    dropped = RrdLoader(test_rrd_path).stream().drop(components="Points3D:positions").collect()
    for chunk in dropped:
        # Chunks that weren't sliced (no Points3D:positions to remove) keep their original id,
        # but the /robots/arm chunk that had the column removed must get a new id.
        if chunk.entity_path == "/robots/arm":
            assert chunk.id not in original_ids, "drop(components=...) must assign a new ChunkId"  # NOLINT


@pytest.mark.local_only
def test_filter_multiple_components(test_rrd_path: Path) -> None:
    """filter(components=[A, B]) keeps both columns when present (OR semantics)."""
    stream = RrdLoader(test_rrd_path).stream()
    chunks = stream.filter(components=["Points3D:positions", "Points3D:colors"]).collect()

    assert len(chunks) > 0
    for chunk in chunks:
        assert chunk.entity_path == "/robots/arm"


@pytest.mark.local_only
def test_filter_multiple_components_partial(test_rrd_path: Path) -> None:
    """filter(components=[A, Z]) where Z doesn't exist: keep A only."""
    stream = RrdLoader(test_rrd_path).stream()
    chunks = stream.filter(components=["Points3D:positions", "Nonexistent:foo"]).collect()

    assert len(chunks) > 0
    for chunk in chunks:
        assert chunk.entity_path == "/robots/arm"


@pytest.mark.local_only
def test_filter_multiple_components_none_present(test_rrd_path: Path) -> None:
    """filter(components=[Z1, Z2]) where neither exist: chunk dropped."""
    stream = RrdLoader(test_rrd_path).stream()
    chunks = stream.filter(components=["Nonexistent:a", "Nonexistent:b"]).collect()
    assert len(chunks) == 0


@pytest.mark.local_only
def test_drop_multiple_components(test_rrd_path: Path) -> None:
    """drop(components=[A, B]) removes both columns."""
    stream = RrdLoader(test_rrd_path).stream()
    # Drop both component columns from /robots/arm — those chunks should be dropped
    # since they have no remaining component columns.
    all_chunks = stream.collect()
    arm_chunk_count = sum(1 for c in all_chunks if c.entity_path == "/robots/arm")

    stream2 = RrdLoader(test_rrd_path).stream()
    dropped = stream2.drop(components=["Points3D:positions", "Points3D:colors"]).collect()

    # Arm chunks that had ONLY those components are dropped; others survive.
    arm_dropped = [c for c in dropped if c.entity_path == "/robots/arm"]
    assert len(arm_dropped) < arm_chunk_count


@pytest.mark.local_only
def test_split_multiple_components(test_rrd_path: Path) -> None:
    """split(components=[A, B]): matched gets A+B, complement gets rest."""
    stream = RrdLoader(test_rrd_path).stream()
    matched, complement = stream.split(components=["Points3D:positions", "Points3D:colors"])

    matched_chunks = matched.collect()
    complement_chunks = complement.collect()

    # Matched should have the arm entity chunks with positions+colors
    assert len(matched_chunks) > 0
    for chunk in matched_chunks:
        assert chunk.entity_path == "/robots/arm"

    # Complement should have everything that wasn't matched
    assert len(complement_chunks) > 0


# ---------------------------------------------------------------------------
# drop
# ---------------------------------------------------------------------------


@pytest.mark.local_only
def test_drop(test_rrd_path: Path) -> None:
    stream = RrdLoader(test_rrd_path).stream()
    chunks = non_property_chunks(stream.drop(content="/robots/**").collect())

    assert "/robots/arm" not in entity_paths(chunks)
    assert "/cameras/front" in entity_paths(chunks)
    assert "/config" in entity_paths(chunks)


# ---------------------------------------------------------------------------
# split / merge
# ---------------------------------------------------------------------------


@pytest.mark.local_only
def test_split_merge_roundtrip(test_rrd_path: Path) -> None:
    stream = RrdLoader(test_rrd_path).stream()
    original = non_property_chunks(stream.collect())
    original_count = len(original)

    stream2 = RrdLoader(test_rrd_path).stream()
    static_branch, temporal_branch = stream2.split(is_static=True)
    merged = non_property_chunks(LazyChunkStream.merge(static_branch, temporal_branch).collect())

    assert len(merged) == original_count
    assert entity_paths(merged) == entity_paths(original)


@pytest.mark.local_only
def test_split_drop_one_branch(test_rrd_path: Path) -> None:
    """Consuming only one branch of a split should not hang."""
    stream = RrdLoader(test_rrd_path).stream()
    matching, _non_matching = stream.split(content="/robots/**")

    # Only consume matching branch; let _non_matching be GC'd
    chunks = matching.collect()
    assert len(chunks) > 0
    assert all(c.entity_path == "/robots/arm" for c in chunks)


# ---------------------------------------------------------------------------
# from_iter
# ---------------------------------------------------------------------------


@pytest.mark.local_only
def test_from_iter(test_rrd_path: Path) -> None:
    stream = RrdLoader(test_rrd_path).stream()
    original = stream.collect()

    roundtripped = LazyChunkStream.from_iter(original).collect()
    assert len(roundtripped) == len(original)
    assert entity_paths(roundtripped) == entity_paths(original)


# ---------------------------------------------------------------------------
# Composition
# ---------------------------------------------------------------------------


@pytest.mark.local_only
def test_chained_filters(test_rrd_path: Path) -> None:
    stream = RrdLoader(test_rrd_path).stream()
    chunks = non_property_chunks(stream.filter(is_static=False).filter(content="/robots/**").collect())

    assert entity_paths(chunks) == {"/robots/arm"}
    assert all(not c.is_static for c in chunks)


# ---------------------------------------------------------------------------
# Dangling split branch
# ---------------------------------------------------------------------------


@pytest.mark.local_only
def test_dangling_split_matched_only(test_rrd_path: Path, capfd: pytest.CaptureFixture[str]) -> None:
    """Using only the matched branch of a split should work (degenerated to filter) and warn."""
    stream = RrdLoader(test_rrd_path).stream()
    matched, _unmatched = stream.split(content="/robots/**")

    # The dangling branch (_unmatched) is never consumed.
    # This should NOT hang and should produce correct results.
    chunks = matched.collect()

    assert len(chunks) > 0
    assert all(c.entity_path == "/robots/arm" for c in chunks)

    captured = capfd.readouterr()
    assert "only one branch" in captured.err.lower(), (
        f"Expected a warning about dangling split branch on stderr, got: {captured.err!r}"
    )


@pytest.mark.local_only
def test_dangling_split_unmatched_only(test_rrd_path: Path, capfd: pytest.CaptureFixture[str]) -> None:
    """Using only the unmatched branch of a split should work (degenerated to drop) and warn."""
    stream = RrdLoader(test_rrd_path).stream()
    _matched, unmatched = stream.split(content="/robots/**")

    chunks = non_property_chunks(unmatched.collect())
    assert len(chunks) > 0
    assert "/robots/arm" not in entity_paths(chunks)

    captured = capfd.readouterr()
    assert "only one branch" in captured.err.lower(), (
        f"Expected a warning about dangling split branch on stderr, got: {captured.err!r}"
    )


# ---------------------------------------------------------------------------
# Move semantics
# ---------------------------------------------------------------------------


@pytest.mark.local_only
def test_stream_consumed_after_filter(test_rrd_path: Path) -> None:
    """A stream consumed by filter() cannot be used again as a builder input."""
    stream = RrdLoader(test_rrd_path).stream()
    _filtered = stream.filter(is_static=True)

    with pytest.raises(ValueError, match="already been consumed"):
        stream.drop(is_static=False)


@pytest.mark.local_only
def test_stream_consumed_after_split(test_rrd_path: Path) -> None:
    """A stream consumed by split() cannot be used again as a builder input."""
    stream = RrdLoader(test_rrd_path).stream()
    _a, _b = stream.split(is_static=True)

    with pytest.raises(ValueError, match="already been consumed"):
        stream.filter(content="/foo/**")


@pytest.mark.local_only
def test_merge_same_stream_twice(test_rrd_path: Path) -> None:
    """Passing the same stream to merge twice is an error."""
    stream = RrdLoader(test_rrd_path).stream()
    a, _b = stream.split(is_static=True)

    with pytest.raises(ValueError, match="already been consumed"):
        LazyChunkStream.merge(a, a)


@pytest.mark.local_only
def test_merge_indirect_reuse(test_rrd_path: Path) -> None:
    """A stream used as split upstream and also passed directly to merge is an error."""
    stream = RrdLoader(test_rrd_path).stream()
    a, b = stream.split(is_static=True)
    _b1, b2 = b.split(content="/robots/**")

    # b was consumed by the second split, so passing it to merge should fail.
    with pytest.raises(ValueError, match="already been consumed"):
        LazyChunkStream.merge(b, b2, a)


@pytest.mark.local_only
def test_terminal_does_not_consume(test_rrd_path: Path) -> None:
    """Terminals (collect, write_rrd, iter) borrow without consuming."""
    stream = RrdLoader(test_rrd_path).stream()

    chunks1 = stream.collect()
    chunks2 = stream.collect()
    assert len(chunks1) == len(chunks2)

    chunks3 = list(stream)
    assert len(chunks3) == len(chunks1)

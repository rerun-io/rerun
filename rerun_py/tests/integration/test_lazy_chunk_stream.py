"""Tests for rerun.experimental.LazyChunkStream and RrdReader."""

from __future__ import annotations

import subprocess
from typing import TYPE_CHECKING

import pyarrow.compute as pc
import pytest
import rerun as rr
from inline_snapshot import snapshot as inline_snapshot
from rerun.experimental import Chunk, LazyChunkStream, Lens, LensOutput, RrdReader, Selector

from .conftest import TEST_APP_ID as APP_ID, TEST_RECORDING_ID as RECORDING_ID

if TYPE_CHECKING:
    from pathlib import Path


# ---------------------------------------------------------------------------
# RrdReader basics
# ---------------------------------------------------------------------------


def test_rrd_reader_properties(test_rrd_path: Path) -> None:
    reader = RrdReader(test_rrd_path)
    assert reader.application_id == APP_ID
    assert reader.recording_id == RECORDING_ID


def test_rrd_reader_file_not_found(tmp_path: Path) -> None:
    with pytest.raises(ValueError, match="not found"):
        RrdReader(tmp_path / "nonexistent.rrd")


# ---------------------------------------------------------------------------
# to_chunks / iter
# ---------------------------------------------------------------------------


def test_to_chunks(test_rrd_path: Path) -> None:
    """to_chunks() returns Chunk objects with expected properties."""
    chunks = RrdReader(test_rrd_path).stream().to_chunks()

    assert len(chunks) > 0
    for chunk in chunks:
        assert isinstance(chunk, Chunk)
        assert chunk.num_rows > 0
        chunk.format(redact=True)


def test_iter(test_rrd_path: Path) -> None:
    stream = RrdReader(test_rrd_path).stream()
    collected = stream.to_chunks()

    stream2 = RrdReader(test_rrd_path).stream()
    iterated = list(stream2)

    assert len(iterated) == len(collected)


# ---------------------------------------------------------------------------
# collect
# ---------------------------------------------------------------------------


def test_collect_returns_chunk_store(test_rrd_path: Path) -> None:
    """collect() returns a ChunkStore with correct entity paths."""
    store = RrdReader(test_rrd_path).stream().collect()
    paths = store.schema().entity_paths()
    assert "/robots/arm" in paths
    assert "/cameras/front" in paths
    assert "/config" in paths


# ---------------------------------------------------------------------------
# Identity roundtrip
# ---------------------------------------------------------------------------


def test_identity_roundtrip(test_rrd_path: Path, tmp_path: Path) -> None:
    reader = RrdReader(test_rrd_path)
    out = tmp_path / "roundtrip.rrd"

    reader.stream().write_rrd(out, application_id=APP_ID, recording_id=RECORDING_ID)

    original = reader.store()
    roundtripped = RrdReader(out).store()
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
    store = RrdReader(test_rrd_path).stream().filter(content="/robots/**").collect()
    assert store.schema().entity_paths() == ["/robots/arm"]

    # List of strings
    store2 = RrdReader(test_rrd_path).stream().filter(content=["/robots/**", "/cameras/**"]).collect()
    assert store2.schema().entity_paths() == ["/cameras/front", "/robots/arm"]


def test_filter_is_static(test_rrd_path: Path) -> None:
    static_store = RrdReader(test_rrd_path).stream().filter(is_static=True).collect()
    assert static_store.schema().entity_paths() == ["/config"]

    temporal_store = RrdReader(test_rrd_path).stream().filter(is_static=False).collect()
    assert "/config" not in temporal_store.schema().entity_paths()


def test_filter_has_timeline(test_rrd_path: Path) -> None:
    store = RrdReader(test_rrd_path).stream().filter(has_timeline="other_timeline").collect()
    # Only /robots/arm has the other_timeline
    assert store.schema().entity_paths() == ["/robots/arm"]


def test_filter_component(test_rrd_path: Path) -> None:
    store = RrdReader(test_rrd_path).stream().filter(components="Points3D:positions").collect()
    assert store.schema().entity_paths() == ["/robots/arm"]
    # colors column should be stripped — only positions remains
    arm_cols = store.schema().columns_for(entity_path="/robots/arm")
    assert len(arm_cols) == 1
    assert arm_cols[0].component == "Points3D:positions"


def test_component_slice_gets_new_chunk_id(test_rrd_path: Path) -> None:
    """Slicing by component must produce chunks with fresh IDs, not reuse the original."""
    original_ids = {c.id for c in RrdReader(test_rrd_path).stream().to_chunks()}

    # filter keeps only the matching column -> sliced chunk
    filtered = RrdReader(test_rrd_path).stream().filter(components="Points3D:positions").to_chunks()
    for chunk in filtered:
        assert chunk.id not in original_ids, "filter(components=...) must assign a new ChunkId"  # NOLINT

    # drop keeps the non-matching columns -> also a sliced chunk
    dropped = RrdReader(test_rrd_path).stream().drop(components="Points3D:positions").to_chunks()
    for chunk in dropped:
        if chunk.entity_path == "/robots/arm":
            assert chunk.id not in original_ids, "drop(components=...) must assign a new ChunkId"  # NOLINT


def test_filter_multiple_components(test_rrd_path: Path) -> None:
    """filter(components=[A, B]) keeps both columns when present (OR semantics)."""
    store = RrdReader(test_rrd_path).stream().filter(components=["Points3D:positions", "Points3D:colors"]).collect()
    assert store.schema().entity_paths() == ["/robots/arm"]
    arm_cols = store.schema().columns_for(entity_path="/robots/arm")
    assert len(arm_cols) == 2


def test_filter_multiple_components_partial(test_rrd_path: Path) -> None:
    """filter(components=[A, Z]) where Z doesn't exist: keep A only."""
    store = RrdReader(test_rrd_path).stream().filter(components=["Points3D:positions", "Nonexistent:foo"]).collect()
    assert store.schema().entity_paths() == ["/robots/arm"]


def test_filter_multiple_components_none_present(test_rrd_path: Path) -> None:
    """filter(components=[Z1, Z2]) where neither exist: empty store."""
    store = RrdReader(test_rrd_path).stream().filter(components=["Nonexistent:a", "Nonexistent:b"]).collect()
    assert store.schema().entity_paths() == []


def test_drop_multiple_components(test_rrd_path: Path) -> None:
    """drop(components=[A, B]) removes both columns."""
    store = RrdReader(test_rrd_path).stream().drop(components=["Points3D:positions", "Points3D:colors"]).collect()
    # /robots/arm had only those two components, so it should be gone
    assert "/robots/arm" not in store.schema().entity_paths()


def test_split_multiple_components(test_rrd_path: Path) -> None:
    """split(components=[A, B]): matched gets A+B, complement gets rest."""
    stream = RrdReader(test_rrd_path).stream()
    matched, complement = stream.split(components=["Points3D:positions", "Points3D:colors"])

    matched_store = matched.collect()
    complement_store = complement.collect()

    assert matched_store.schema().entity_paths() == ["/robots/arm"]
    assert len(complement_store.schema().entity_paths()) > 0


# ---------------------------------------------------------------------------
# drop
# ---------------------------------------------------------------------------


def test_drop(test_rrd_path: Path) -> None:
    store = RrdReader(test_rrd_path).stream().drop(content="/robots/**").collect()
    paths = store.schema().entity_paths()
    assert "/robots/arm" not in paths
    assert "/cameras/front" in paths
    assert "/config" in paths


# ---------------------------------------------------------------------------
# split / merge
# ---------------------------------------------------------------------------


def test_split_merge_roundtrip(test_rrd_path: Path) -> None:
    original = RrdReader(test_rrd_path).stream().collect()

    stream2 = RrdReader(test_rrd_path).stream()
    static_branch, temporal_branch = stream2.split(is_static=True)
    merged = LazyChunkStream.merge(static_branch, temporal_branch).collect()

    assert original.schema() == merged.schema()


def test_split_drop_one_branch(test_rrd_path: Path) -> None:
    """Consuming only one branch of a split should not hang."""
    stream = RrdReader(test_rrd_path).stream()
    matching, _non_matching = stream.split(content="/robots/**")

    store = matching.collect()
    assert store.schema().entity_paths() == ["/robots/arm"]


# ---------------------------------------------------------------------------
# from_iter
# ---------------------------------------------------------------------------


def test_from_iter(test_rrd_path: Path) -> None:
    original = RrdReader(test_rrd_path).stream().to_chunks()

    roundtripped = LazyChunkStream.from_iter(original).collect()
    assert roundtripped.schema() == RrdReader(test_rrd_path).store().schema()


# ---------------------------------------------------------------------------
# Composition
# ---------------------------------------------------------------------------


def test_chained_filters(test_rrd_path: Path) -> None:
    store = RrdReader(test_rrd_path).stream().filter(is_static=False).filter(content="/robots/**").collect()
    assert store.schema().entity_paths() == ["/robots/arm"]


# ---------------------------------------------------------------------------
# Dangling split branch
# ---------------------------------------------------------------------------


def test_dangling_split_matched_only(test_rrd_path: Path, capfd: pytest.CaptureFixture[str]) -> None:
    """Using only the matched branch of a split should work (degenerated to filter) and warn."""
    stream = RrdReader(test_rrd_path).stream()
    matched, _unmatched = stream.split(content="/robots/**")

    store = matched.collect()
    assert store.schema().entity_paths() == ["/robots/arm"]

    captured = capfd.readouterr()
    assert "only one branch" in captured.err.lower(), (
        f"Expected a warning about dangling split branch on stderr, got: {captured.err!r}"
    )


def test_dangling_split_unmatched_only(test_rrd_path: Path, capfd: pytest.CaptureFixture[str]) -> None:
    """Using only the unmatched branch of a split should work (degenerated to drop) and warn."""
    stream = RrdReader(test_rrd_path).stream()
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
    stream = RrdReader(test_rrd_path).stream()
    _filtered = stream.filter(is_static=True)

    with pytest.raises(ValueError, match="already been consumed"):
        stream.drop(is_static=False)


def test_stream_consumed_after_split(test_rrd_path: Path) -> None:
    """A stream consumed by split() cannot be used again as a builder input."""
    stream = RrdReader(test_rrd_path).stream()
    _a, _b = stream.split(is_static=True)

    with pytest.raises(ValueError, match="already been consumed"):
        stream.filter(content="/foo/**")


def test_merge_same_stream_twice(test_rrd_path: Path) -> None:
    """Passing the same stream to merge twice is an error."""
    stream = RrdReader(test_rrd_path).stream()
    a, _b = stream.split(is_static=True)

    with pytest.raises(ValueError, match="already been consumed"):
        LazyChunkStream.merge(a, a)


def test_merge_indirect_reuse(test_rrd_path: Path) -> None:
    """A stream used as split upstream and also passed directly to merge is an error."""
    stream = RrdReader(test_rrd_path).stream()
    a, b = stream.split(is_static=True)
    _b1, b2 = b.split(content="/robots/**")

    # b was consumed by the second split, so passing it to merge should fail.
    with pytest.raises(ValueError, match="already been consumed"):
        LazyChunkStream.merge(b, b2, a)


def test_terminal_does_not_consume(test_rrd_path: Path) -> None:
    """Terminals (collect, write_rrd, iter) borrow without consuming."""
    stream = RrdReader(test_rrd_path).stream()

    store1 = stream.collect()
    store2 = stream.collect()
    assert store1.schema() == store2.schema()

    chunks = list(stream)
    assert len(chunks) > 0


# ---------------------------------------------------------------------------
# Lenses
# ---------------------------------------------------------------------------


def test_lenses_identity(test_rrd_path: Path) -> None:
    """A lens with Selector('.') passes through the struct component data unchanged."""

    lens = Lens(
        "Imu:accel",
        LensOutput().to_component("Imu:accel", Selector(".")),
    )

    store = RrdReader(test_rrd_path).stream().filter(content="/sensors/**").lenses(lens).collect()
    assert store.summary() == inline_snapshot(
        "/sensors/imu rows=2 bytes=1.6 KiB static=False timelines=['my_index'] cols=['Imu:accel', 'my_index']"
    )


def test_lenses_field_selector(test_rrd_path: Path) -> None:
    """A lens with Selector('.x') extracts a struct field and reinterprets it as a Rerun Scalar."""

    lens = Lens(
        "Imu:accel",
        LensOutput().to_component(rr.Scalars.descriptor_scalars(), Selector(".x")),
    )

    store = RrdReader(test_rrd_path).stream().filter(content="/sensors/**").lenses(lens).collect()
    assert store.summary() == inline_snapshot(
        "/sensors/imu rows=2 bytes=1.5 KiB static=False timelines=['my_index'] cols=['Scalars:scalars', 'my_index']"
    )

    # Verify the extracted values are correct
    chunks = RrdReader(test_rrd_path).stream().filter(content="/sensors/**").lenses(lens).to_chunks()
    rb = chunks[0].to_record_batch()
    scalars = rb.column("Scalars:scalars")
    assert scalars.to_pylist() == [[0.1], [0.4]]


def test_lenses_multiple_outputs(test_rrd_path: Path) -> None:
    """A single lens can produce multiple output groups at different entity paths."""

    lens = Lens(
        "Imu:accel",
        to_entity={
            "/out/x": LensOutput().to_component(rr.Scalars.descriptor_scalars(), Selector(".x")),
            "/out/z": LensOutput().to_component(rr.Scalars.descriptor_scalars(), Selector(".z")),
        },
    )

    store = RrdReader(test_rrd_path).stream().filter(content="/sensors/**").lenses(lens).collect()
    assert store.summary() == inline_snapshot("""\
/out/x rows=2 bytes=1.5 KiB static=False timelines=['my_index'] cols=['Scalars:scalars', 'my_index']
/out/z rows=2 bytes=1.5 KiB static=False timelines=['my_index'] cols=['Scalars:scalars', 'my_index']\
""")


def test_lenses_drop_unmatched(test_rrd_path: Path) -> None:
    """With drop_unmatched (default), unmatched chunks are not forwarded."""

    lens = Lens(
        "nonexistent:Component:foo",
        LensOutput().to_component("out:Component:bar", Selector(".")),
    )

    store = RrdReader(test_rrd_path).stream().lenses(lens, output_mode="drop_unmatched").collect()
    assert store.summary() == inline_snapshot("")


def test_lenses_forward_unmatched(test_rrd_path: Path) -> None:
    """With forward_unmatched, transformed chunks replace originals and unmatched chunks pass through."""

    lens = Lens(
        "Imu:accel",
        to_entity={
            "/transformed": LensOutput().to_component(rr.Scalars.descriptor_scalars(), Selector(".x")),
        },
    )

    store = (
        RrdReader(test_rrd_path)
        .stream()
        .lenses(lens, output_mode="forward_unmatched")
        .drop(content="/__properties/**")
        .collect()
    )
    assert store.summary() == inline_snapshot("""\
/cameras/front rows=1 bytes=1.5 KiB static=False timelines=['my_index'] cols=['TextLog:text', 'my_index']
/config rows=1 bytes=1.1 KiB static=True timelines=[] cols=['TextLog:text']
/robots/arm rows=2 bytes=1.6 KiB static=False timelines=['my_index', 'other_timeline'] cols=['Points3D:colors', 'Points3D:positions', 'my_index', 'other_timeline']
/transformed rows=2 bytes=1.5 KiB static=False timelines=['my_index'] cols=['Scalars:scalars', 'my_index']\
""")


def test_lenses_forward_all(test_rrd_path: Path) -> None:
    """With forward_all, both transformed and original data are forwarded."""

    lens = Lens(
        "Imu:accel",
        to_entity={
            "/transformed": LensOutput().to_component(rr.Scalars.descriptor_scalars(), Selector(".x")),
        },
    )

    store = (
        RrdReader(test_rrd_path)
        .stream()
        .lenses(lens, output_mode="forward_all")
        .drop(content="/__properties/**")
        .collect()
    )
    assert store.summary() == inline_snapshot("""\
/cameras/front rows=1 bytes=1.5 KiB static=False timelines=['my_index'] cols=['TextLog:text', 'my_index']
/config rows=1 bytes=1.1 KiB static=True timelines=[] cols=['TextLog:text']
/robots/arm rows=2 bytes=1.6 KiB static=False timelines=['my_index', 'other_timeline'] cols=['Points3D:colors', 'Points3D:positions', 'my_index', 'other_timeline']
/sensors/imu rows=2 bytes=1.6 KiB static=False timelines=['my_index'] cols=['Imu:accel', 'my_index']
/transformed rows=2 bytes=1.5 KiB static=False timelines=['my_index'] cols=['Scalars:scalars', 'my_index']\
""")


def test_lenses_consumes_stream(test_rrd_path: Path) -> None:
    """Calling .lenses() consumes the stream (move semantics)."""

    lens = Lens(
        "Imu:accel",
        LensOutput().to_component(rr.Scalars.descriptor_scalars(), Selector(".x")),
    )

    stream = RrdReader(test_rrd_path).stream()
    _transformed = stream.lenses(lens)

    with pytest.raises(ValueError, match="already been consumed"):
        stream.filter(is_static=True)


def test_lenses_chained_with_filter(test_rrd_path: Path) -> None:
    """Lenses can be composed with filter in a pipeline."""

    lens = Lens(
        "Imu:accel",
        LensOutput().to_component(rr.Scalars.descriptor_scalars(), Selector(".z")),
    )

    store = RrdReader(test_rrd_path).stream().filter(content="/sensors/**").lenses(lens).collect()
    assert store.summary() == inline_snapshot(
        "/sensors/imu rows=2 bytes=1.5 KiB static=False timelines=['my_index'] cols=['Scalars:scalars', 'my_index']"
    )


def test_lenses_invalid_output_mode(test_rrd_path: Path) -> None:
    """Invalid output_mode string raises ValueError."""

    lens = Lens(
        "Points3D:positions",
        LensOutput().to_component("Points3D:positions", Selector(".")),
    )

    with pytest.raises(ValueError, match="Unknown output_mode"):
        RrdReader(test_rrd_path).stream().lenses(lens, output_mode="invalid")  # type: ignore[arg-type]


def test_lenses_time_extraction(test_rrd_path: Path) -> None:
    """A lens can extract a timestamp field from a struct component as a new timeline."""

    lens = Lens(
        "Imu:accel",
        LensOutput()
        .to_component(rr.Scalars.descriptor_scalars(), Selector(".x"))
        .to_timeline("sensor_time", "timestamp_ns", Selector(".timestamp")),
    )

    store = RrdReader(test_rrd_path).stream().filter(content="/sensors/**").lenses(lens).collect()
    assert store.summary() == inline_snapshot(
        "/sensors/imu rows=2 bytes=1.5 KiB static=False timelines=['my_index', 'sensor_time'] cols=['Scalars:scalars', 'my_index', 'sensor_time']"
    )

    chunks = RrdReader(test_rrd_path).stream().filter(content="/sensors/**").lenses(lens).to_chunks()
    rb = chunks[0].to_record_batch()
    scalars = rb.column("Scalars:scalars")
    assert scalars.to_pylist() == [[0.1], [0.4]]

    sensor_time = rb.column("sensor_time")
    assert [t.value for t in sensor_time.to_pylist()] == [1000000000, 2000000000]


def test_lenses_dynamic_selector(test_rrd_path: Path) -> None:
    """A lens with a dynamic selector uses .pipe() to transform data with a Python callable."""

    selector = Selector(".x").pipe(lambda arr: pc.multiply(arr, 2.0))

    lens = Lens(
        "Imu:accel",
        LensOutput().to_component(rr.Scalars.descriptor_scalars(), selector),
    )

    store = RrdReader(test_rrd_path).stream().filter(content="/sensors/**").lenses(lens).collect()
    assert store.summary() == inline_snapshot(
        "/sensors/imu rows=2 bytes=1.5 KiB static=False timelines=['my_index'] cols=['Scalars:scalars', 'my_index']"
    )

    chunks = RrdReader(test_rrd_path).stream().filter(content="/sensors/**").lenses(lens).to_chunks()
    rb = chunks[0].to_record_batch()
    scalars = rb.column("Scalars:scalars")
    assert scalars.to_pylist() == [[0.2], [0.8]]


# ---------------------------------------------------------------------------
# map / flat_map
# ---------------------------------------------------------------------------


def test_map_identity(test_rrd_path: Path) -> None:
    """map(identity) preserves all chunks."""
    original = RrdReader(test_rrd_path).stream().to_chunks()
    mapped = RrdReader(test_rrd_path).stream().map(lambda c: c).to_chunks()
    assert len(mapped) == len(original)


def test_flat_map_identity(test_rrd_path: Path) -> None:
    """flat_map(lambda c: [c]) preserves all chunks."""
    original = RrdReader(test_rrd_path).stream().to_chunks()
    mapped = RrdReader(test_rrd_path).stream().flat_map(lambda c: [c]).to_chunks()
    assert len(mapped) == len(original)


def test_flat_map_drop_all(test_rrd_path: Path) -> None:
    """flat_map(lambda c: []) produces an empty stream."""
    chunks = RrdReader(test_rrd_path).stream().flat_map(lambda _c: []).to_chunks()
    assert len(chunks) == 0


def test_map_after_filter(test_rrd_path: Path) -> None:
    """Map composes after filter."""
    store = RrdReader(test_rrd_path).stream().filter(content="/robots/**").map(lambda c: c).collect()
    assert store.schema().entity_paths() == ["/robots/arm"]


def test_map_in_split_branch(test_rrd_path: Path) -> None:
    """Map works on a branch produced by split."""
    stream = RrdReader(test_rrd_path).stream()
    matched, _non_matched = stream.split(content="/robots/**")
    mapped = matched.map(lambda c: c)
    store = mapped.collect()
    assert store.schema().entity_paths() == ["/robots/arm"]


def test_map_error_propagation(test_rrd_path: Path) -> None:
    """A map callable that raises propagates the exception."""

    def _raise(_c: Chunk) -> Chunk:
        raise ValueError("test error from map")

    with pytest.raises(ValueError, match="test error from map"):
        RrdReader(test_rrd_path).stream().map(_raise).to_chunks()


def test_flat_map_error_propagation(test_rrd_path: Path) -> None:
    """A flat_map callable that raises propagates the exception."""

    def _raise(_c: Chunk) -> list[Chunk]:
        raise ValueError("test error from flat_map")

    with pytest.raises(ValueError, match="test error from flat_map"):
        RrdReader(test_rrd_path).stream().flat_map(_raise).to_chunks()


def test_map_consumed_stream(test_rrd_path: Path) -> None:
    """Calling map on a consumed stream raises ValueError."""
    stream = RrdReader(test_rrd_path).stream()
    _filtered = stream.filter(is_static=True)
    with pytest.raises(ValueError, match="already been consumed"):
        stream.map(lambda c: c)


def test_flat_map_consumed_stream(test_rrd_path: Path) -> None:
    """Calling flat_map on a consumed stream raises ValueError."""
    stream = RrdReader(test_rrd_path).stream()
    _filtered = stream.filter(is_static=True)
    with pytest.raises(ValueError, match="already been consumed"):
        stream.flat_map(lambda c: [c])


def test_map_multiple_executions(test_rrd_path: Path) -> None:
    """Calling to_chunks() twice on a mapped stream produces the same results."""
    stream = RrdReader(test_rrd_path).stream().map(lambda c: c)
    first = stream.to_chunks()
    second = stream.to_chunks()
    assert len(first) == len(second)

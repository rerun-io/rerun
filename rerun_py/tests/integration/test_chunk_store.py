"""Integration tests for ChunkStore."""

from __future__ import annotations

import pathlib
from typing import TYPE_CHECKING

import pytest
import rerun as rr
from inline_snapshot import snapshot as inline_snapshot
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


VIDEO_ASSETS_DIR = pathlib.Path(__file__).parents[3] / "tests" / "assets" / "video"

# (filename, rerun codec) pairs exercised by the VideoStream compaction test.
VIDEO_CASES = [
    ("Big_Buck_Bunny_1080_10s_av1.mp4", rr.VideoCodec.AV1),
    ("Big_Buck_Bunny_1080_1s_h264_nobframes.mp4", rr.VideoCodec.H264),
    ("Sintel_1080_10s_av1.mp4", rr.VideoCodec.AV1),
]


def _build_video_stream_rrd(tmp_dir: Path, filename: str, codec: rr.VideoCodec) -> tuple[Path, int]:
    """
    Build an RRD with one VideoStream sample chunk per demuxed mp4 packet.

    Returns ``(rrd_path, num_gops)`` where ``num_gops`` is the number of
    keyframes (I-frames) in the source video.
    """
    import av
    from av.bitstream import BitStreamFilterContext

    video_path = VIDEO_ASSETS_DIR / filename
    rrd_path = tmp_dir / f"{video_path.stem}.rrd"

    container = av.open(str(video_path))
    num_gops = 0
    try:
        video_stream = container.streams.video[0]
        time_base = video_stream.time_base
        assert time_base is not None

        # Rerun's VideoStream expects AnnexB for H.264/H.265, but mp4 demuxing yields
        # AVCC-style length-prefixed NALs. Apply the matching bitstream filter so
        # `re_video::is_start_of_gop` can parse the samples without spamming errors.
        filter_name = {
            rr.VideoCodec.H264: "h264_mp4toannexb",
            rr.VideoCodec.H265: "hevc_mp4toannexb",
        }.get(codec)
        bsf = BitStreamFilterContext(filter_name, video_stream) if filter_name else None

        with rr.RecordingStream("rerun_example_video_compact", recording_id=f"video-compact-{video_path.stem}") as rec:
            rec.save(rrd_path)
            rec.log("/video", rr.VideoStream(codec=codec), static=True)

            def log_packet(packet: av.Packet) -> None:
                nonlocal num_gops
                if packet.pts is None or packet.size == 0:
                    return
                if packet.is_keyframe:
                    num_gops += 1
                pts_seconds = float(packet.pts * time_base)
                rec.send_columns(
                    "/video",
                    indexes=[rr.TimeColumn("video_time", duration=[pts_seconds])],
                    columns=rr.VideoStream.columns(sample=[bytes(packet)]),
                )

            for packet in container.demux(video_stream):
                if bsf is None:
                    log_packet(packet)
                else:
                    for out in bsf.filter(packet):
                        log_packet(out)
    finally:
        container.close()

    return rrd_path, num_gops


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


def test_compact_video_stream_summary(tmp_path_factory: pytest.TempPathFactory) -> None:
    """Snapshot the summary of a VideoStream recording: normal compaction vs + GoP batching."""

    def report(label: str, num_gops: int, s: ChunkStore) -> str:
        num_chunks = sum(1 for _ in s.stream().to_chunks())
        return f"{label}: num_gops={num_gops} num_chunks={num_chunks}\n{s.summary()}"

    sections = []
    for filename, codec in VIDEO_CASES:
        tmp_dir = tmp_path_factory.mktemp("compact_video")
        rrd_path, num_gops = _build_video_stream_rrd(tmp_dir, filename, codec)
        store = RrdReader(rrd_path).store()

        # Normal compaction only (no GoP alignment).
        without_gop = store.compact(gop_batching=False)
        # Then re-compact with GoP batching on top.
        with_gop = without_gop.compact(gop_batching=True)

        sections.append(f"=== {filename} ===")
        sections.append(report("before_gop", num_gops, without_gop))
        sections.append(report("after_gop", num_gops, with_gop))
        sections.append("\n")

    assert "\n".join(sections) == inline_snapshot("""\
=== Big_Buck_Bunny_1080_10s_av1.mp4 ===
before_gop: num_gops=1 num_chunks=17
/video rows=1 bytes=1.1 KiB static=True timelines=[] cols=['VideoStream:codec']
/video rows=1 bytes=534 KiB static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=19 bytes=378 KiB static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=19 bytes=382 KiB static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=24 bytes=378 KiB static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=22 bytes=330 KiB static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=17 bytes=277 KiB static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=17 bytes=279 KiB static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=17 bytes=280 KiB static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=18 bytes=381 KiB static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=32 bytes=377 KiB static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=18 bytes=278 KiB static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=19 bytes=377 KiB static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=16 bytes=297 KiB static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=31 bytes=371 KiB static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=30 bytes=237 KiB static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/__properties rows=1 bytes=1.1 KiB static=True timelines=[] cols=['RecordingInfo:start_time']
after_gop: num_gops=1 num_chunks=3
/video rows=1 bytes=1.1 KiB static=True timelines=[] cols=['VideoStream:codec']
/video rows=315 bytes=6.4 MiB static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/__properties rows=1 bytes=1.1 KiB static=True timelines=[] cols=['RecordingInfo:start_time']


=== Big_Buck_Bunny_1080_1s_h264_nobframes.mp4 ===
before_gop: num_gops=1 num_chunks=11
/video rows=1 bytes=1.1 KiB static=True timelines=[] cols=['VideoStream:codec']
/video rows=1 bytes=348 KiB static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=4 bytes=353 KiB static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=4 bytes=371 KiB static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=3 bytes=293 KiB static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=3 bytes=297 KiB static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=4 bytes=379 KiB static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=3 bytes=302 KiB static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=4 bytes=379 KiB static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=4 bytes=260 KiB static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/__properties rows=1 bytes=1.1 KiB static=True timelines=[] cols=['RecordingInfo:start_time']
after_gop: num_gops=1 num_chunks=3
/video rows=1 bytes=1.1 KiB static=True timelines=[] cols=['VideoStream:codec']
/video rows=39 bytes=4.0 MiB static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/__properties rows=1 bytes=1.1 KiB static=True timelines=[] cols=['RecordingInfo:start_time']


=== Sintel_1080_10s_av1.mp4 ===
before_gop: num_gops=12 num_chunks=5
/video rows=1 bytes=1.1 KiB static=True timelines=[] cols=['VideoStream:codec']
/video rows=114 bytes=382 KiB static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=111 bytes=382 KiB static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=75 bytes=279 KiB static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/__properties rows=1 bytes=1.1 KiB static=True timelines=[] cols=['RecordingInfo:start_time']
after_gop: num_gops=12 num_chunks=6
/video rows=1 bytes=1.1 KiB static=True timelines=[] cols=['VideoStream:codec']
/video rows=100 bytes=381 KiB static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=84 bytes=324 KiB static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=52 bytes=216 KiB static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=77 bytes=318 KiB static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/__properties rows=1 bytes=1.1 KiB static=True timelines=[] cols=['RecordingInfo:start_time']

""")

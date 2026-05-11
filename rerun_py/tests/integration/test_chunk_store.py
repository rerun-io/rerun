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
    OptimizationProfile,
    RrdReader,
)

from .conftest import TEST_APP_ID as APP_ID, TEST_RECORDING_ID as RECORDING_ID

if TYPE_CHECKING:
    from pathlib import Path

    from syrupy import SnapshotAssertion

FRAGMENTED_NUM_ROWS = 4_200


@pytest.fixture(scope="session")
def fragmented_rrd_path(tmp_path_factory: pytest.TempPathFactory) -> Path:
    """
    RRD with `FRAGMENTED_NUM_ROWS` sorted scalar rows on /sensor, one chunk per row.

    Row count is sized to be larger than LIVE's `max_rows=4096` ceiling and
    smaller than OBJECT_STORE's `max_rows=65_536`, so the splitter behaves
    visibly differently under the two profiles.

    Uses `ChunkBatcherConfig.ALWAYS_TEST_ONLY()` so the microbatcher cannot coalesce
    sends behind our back: each `send_columns` becomes its own chunk.
    """
    rrd_path = tmp_path_factory.mktemp("compact") / "fragmented.rrd"
    with rr.RecordingStream(
        "rerun_example_compact_test",
        recording_id="compact-test-id",
        batcher_config=rr.ChunkBatcherConfig.ALWAYS_TEST_ONLY(),
    ) as rec:
        rec.save(rrd_path)
        for i in range(FRAGMENTED_NUM_ROWS):
            rec.send_columns(
                "/sensor",
                indexes=[rr.TimeColumn("frame", sequence=[i])],
                columns=rr.Scalars.columns(scalars=[float(i)]),
            )
    return rrd_path


# Session-scoped collected stores: each `collect()` over the fragmented RRD takes
# ~0.5s, and several tests below need the same outputs. Compute once, share across
# tests — they only read from the resulting `ChunkStore`.
@pytest.fixture(scope="session")
def fragmented_default_store(fragmented_rrd_path: Path) -> ChunkStore:
    return RrdReader(fragmented_rrd_path).stream().collect()


@pytest.fixture(scope="session")
def fragmented_optimized_store(fragmented_rrd_path: Path) -> ChunkStore:
    return RrdReader(fragmented_rrd_path).stream().collect(optimize=OptimizationProfile())


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

    Returns `(rrd_path, num_gops)` where `num_gops` is the number of
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


def test_collect_from_rrd_reader(test_rrd_path: Path) -> None:
    """`reader.stream().collect()` returns a fully-materialized ChunkStore."""
    store = RrdReader(test_rrd_path).stream().collect()
    assert isinstance(store, ChunkStore)


def test_repr(test_rrd_path: Path) -> None:
    store = RrdReader(test_rrd_path).stream().collect()
    assert "ChunkStore" in repr(store)


# ---------------------------------------------------------------------------
# ChunkStore.schema()
# ---------------------------------------------------------------------------


def test_schema(test_rrd_path: Path, snapshot: SnapshotAssertion) -> None:
    """schema() returns a Schema matching the stored data."""
    store = RrdReader(test_rrd_path).stream().collect()
    assert repr(store.schema()) == snapshot


def test_schema_entity_paths(test_rrd_path: Path) -> None:
    store = RrdReader(test_rrd_path).stream().collect()
    paths = store.schema().entity_paths()
    assert "/robots/arm" in paths
    assert "/cameras/front" in paths
    assert "/config" in paths


# ---------------------------------------------------------------------------
# ChunkStore.stream()
# ---------------------------------------------------------------------------


def test_stream_returns_lazy_chunk_stream(test_rrd_path: Path) -> None:
    store = RrdReader(test_rrd_path).stream().collect()
    assert isinstance(store.stream(), LazyChunkStream)


def test_stream_is_repeatable(test_rrd_path: Path) -> None:
    """stream() can be called multiple times; each produces the same schema."""
    store = RrdReader(test_rrd_path).stream().collect()
    first = store.stream().collect()
    second = store.stream().collect()
    assert first.schema() == second.schema()


def test_stream_supports_pipeline_ops(test_rrd_path: Path) -> None:
    """Chunks from load().stream() work with filter/collect."""
    store = RrdReader(test_rrd_path).stream().collect()
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
    """write_rrd() -> RrdReader().stream().collect() preserves schema."""
    store1 = RrdReader(test_rrd_path).stream().collect()
    out = tmp_path / "roundtrip.rrd"
    store1.write_rrd(out, application_id=APP_ID, recording_id=RECORDING_ID)

    store2 = RrdReader(out).stream().collect()
    assert store1.schema() == store2.schema()


def test_write_rrd_metadata(test_rrd_path: Path, tmp_path: Path) -> None:
    """write_rrd() writes the provided application_id and recording_id."""
    store = RrdReader(test_rrd_path).stream().collect()
    out = tmp_path / "meta.rrd"
    store.write_rrd(out, application_id="rerun_example_my_app", recording_id="my-rec")

    reader = RrdReader(out)
    recs = reader.recordings()
    assert len(recs) == 1
    assert recs[0].application_id == "rerun_example_my_app"
    assert recs[0].recording_id == "my-rec"


# ---------------------------------------------------------------------------
# LazyChunkStream.collect(compaction=...)
# ---------------------------------------------------------------------------


def test_collect_default_single_pass_compacts(fragmented_default_store: ChunkStore) -> None:
    """Default collect() applies single-pass compaction (what happens on insert)."""
    # Without any optimization, many tiny single-row chunks still get merged by
    # the natural insert-time compaction path.
    assert len(fragmented_default_store.stream().to_chunks()) < FRAGMENTED_NUM_ROWS


def test_collect_optimize_further_reduces(
    fragmented_default_store: ChunkStore,
    fragmented_optimized_store: ChunkStore,
) -> None:
    """Explicit optimize=OptimizationProfile() reduces chunk count further."""
    assert len(fragmented_optimized_store.stream().to_chunks()) <= len(fragmented_default_store.stream().to_chunks())


def test_collect_preserves_schema(
    fragmented_default_store: ChunkStore,
    fragmented_optimized_store: ChunkStore,
) -> None:
    """Optimization preserves the schema."""
    assert fragmented_default_store.schema() == fragmented_optimized_store.schema()


def test_collect_preserves_row_count(
    fragmented_default_store: ChunkStore,
    fragmented_optimized_store: ChunkStore,
) -> None:
    """Optimization preserves the total number of rows."""
    default_rows = sum(c.num_rows for c in fragmented_default_store.stream().to_chunks())
    optimized_rows = sum(c.num_rows for c in fragmented_optimized_store.stream().to_chunks())
    assert optimized_rows == default_rows


def test_collect_with_object_store_profile_uses_object_store_thresholds(
    fragmented_rrd_path: Path,
) -> None:
    """
    End-to-end plumbing: OBJECT_STORE's larger thresholds reach the resulting ChunkStore.

    Proves the precedence chain `OptimizationProfile.OBJECT_STORE → PyO3 → ChunkStoreConfig`
    forwards concrete values (no silent fallback to DEFAULT/LIVE) by checking
    that the `chunk_max_rows` threshold is *enforced* on the /sensor chunks:

    - LIVE caps every chunk at 4096 rows.
    - OBJECT_STORE lets at least one chunk hold more than 4096 rows. If
      OBJECT_STORE's value did not reach the store, splitting would have
      capped it at 4096 too.

    This avoids relying on compaction heuristics converging to a specific
    chunk count: it only relies on the splitter respecting the configured
    ceiling, which is a hard invariant.
    """
    live_store = RrdReader(fragmented_rrd_path).stream().collect(optimize=OptimizationProfile.LIVE)
    object_store_store = RrdReader(fragmented_rrd_path).stream().collect(optimize=OptimizationProfile.OBJECT_STORE)

    def sensor_rows(s: ChunkStore) -> list[int]:
        return [c.num_rows for c in s.stream().to_chunks() if str(c.entity_path) == "/sensor"]

    live_sensor = sensor_rows(live_store)
    object_store_sensor = sensor_rows(object_store_store)

    # Schema and total /sensor row count preserved across profiles.
    assert live_store.schema() == object_store_store.schema()
    assert sum(live_sensor) == sum(object_store_sensor) == FRAGMENTED_NUM_ROWS

    # LIVE enforces its 4096 row ceiling on every chunk.
    assert all(n <= 4096 for n in live_sensor), f"LIVE must respect max_rows=4096: {live_sensor}"

    # OBJECT_STORE's higher 65_536 ceiling lets at least one chunk exceed 4096
    # rows, proving the OBJECT_STORE value reached the store. (If OBJECT_STORE's
    # value were lost, the splitter would have capped chunks at 4096 just like LIVE.)
    assert any(n > 4096 for n in object_store_sensor), (
        f"expected at least one chunk >4096 rows under OBJECT_STORE profile, got {object_store_sensor}"
    )


def test_collect_optimize_video_stream_summary(tmp_path_factory: pytest.TempPathFactory) -> None:
    """Snapshot the summary of a VideoStream recording: optimize without vs with GoP batching."""

    def report(label: str, num_gops: int, s: ChunkStore) -> str:
        num_chunks = sum(1 for _ in s.stream().to_chunks())
        return f"{label}: num_gops={num_gops} num_chunks={num_chunks}\n{s.summary()}"

    sections = []
    for filename, codec in VIDEO_CASES:
        tmp_dir = tmp_path_factory.mktemp("collect_optimize_video")
        rrd_path, num_gops = _build_video_stream_rrd(tmp_dir, filename, codec)
        reader = RrdReader(rrd_path)

        # Optimize without GoP alignment.
        without_gop = reader.stream().collect(optimize=OptimizationProfile(gop_batching=False))
        # Re-optimize with GoP batching on top of the already-optimized store.
        with_gop = without_gop.stream().collect(optimize=OptimizationProfile(gop_batching=True))

        sections.append(f"=== {filename} ===")
        sections.append(report("before_gop", num_gops, without_gop))
        sections.append(report("after_gop", num_gops, with_gop))
        sections.append("\n")

    assert "\n".join(sections) == inline_snapshot("""\
=== Big_Buck_Bunny_1080_10s_av1.mp4 ===
before_gop: num_gops=1 num_chunks=17
/__properties rows=1 static=True timelines=[] cols=['RecordingInfo:start_time']
/video rows=1 static=True timelines=[] cols=['VideoStream:codec']
/video rows=1 static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=19 static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=19 static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=24 static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=22 static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=17 static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=17 static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=17 static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=18 static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=32 static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=18 static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=19 static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=16 static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=31 static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=30 static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
after_gop: num_gops=1 num_chunks=3
/__properties rows=1 static=True timelines=[] cols=['RecordingInfo:start_time']
/video rows=1 static=True timelines=[] cols=['VideoStream:codec']
/video rows=315 static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']


=== Big_Buck_Bunny_1080_1s_h264_nobframes.mp4 ===
before_gop: num_gops=1 num_chunks=11
/__properties rows=1 static=True timelines=[] cols=['RecordingInfo:start_time']
/video rows=1 static=True timelines=[] cols=['VideoStream:codec']
/video rows=1 static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=4 static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=4 static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=3 static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=3 static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=4 static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=3 static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=4 static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=4 static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
after_gop: num_gops=1 num_chunks=3
/__properties rows=1 static=True timelines=[] cols=['RecordingInfo:start_time']
/video rows=1 static=True timelines=[] cols=['VideoStream:codec']
/video rows=39 static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']


=== Sintel_1080_10s_av1.mp4 ===
before_gop: num_gops=12 num_chunks=5
/__properties rows=1 static=True timelines=[] cols=['RecordingInfo:start_time']
/video rows=1 static=True timelines=[] cols=['VideoStream:codec']
/video rows=114 static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=111 static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=75 static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
after_gop: num_gops=12 num_chunks=6
/__properties rows=1 static=True timelines=[] cols=['RecordingInfo:start_time']
/video rows=1 static=True timelines=[] cols=['VideoStream:codec']
/video rows=100 static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=84 static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=52 static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']
/video rows=77 static=False timelines=['video_time'] cols=['VideoStream:sample', 'video_time']

""")

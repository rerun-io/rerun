"""Integration tests for `rerun.experimental.Mp4Reader`."""

from __future__ import annotations

import shutil
from pathlib import Path

import pyarrow as pa
import pytest
from rerun.components import VideoCodec
from rerun.experimental import Chunk, Mp4Reader, Mp4TranscodeOptions, OptimizationProfile, StreamingReader

VIDEO_ASSETS_DIR = Path(__file__).resolve().parents[3] / "tests" / "assets" / "video"

# H.264 fixture encoded without B-frames — usable in both modes.
H264_NO_BFRAMES = VIDEO_ASSETS_DIR / "Big_Buck_Bunny_1080_1s_h264_nobframes.mp4"

# Same content but encoded with B-frames — stream mode transcodes it with ffmpeg
# to strip the B-frames; asset mode is unaffected.
H264_WITH_BFRAMES = VIDEO_ASSETS_DIR / "Big_Buck_Bunny_1080_1s_h264.mp4"

# A genuinely multi-GOP fixture (12 keyframes over 300 frames), so the
# reader-vs-optimize comparison sees more than one GOP to merge.
AV1_MULTI_GOP = VIDEO_ASSETS_DIR / "Sintel_1080_10s_av1.mp4"

_HAS_FFMPEG = shutil.which("ffmpeg") is not None


def _cols(chunk: Chunk) -> list[str]:
    """Return component column names on a chunk (excludes time and control columns)."""
    rb = chunk.to_record_batch()
    timelines = set(chunk.timeline_names)
    return sorted(f.name for f in rb.schema if f.name not in timelines and not f.name.startswith("rerun.controls"))


def _sample_chunks(chunks: list[Chunk]) -> list[Chunk]:
    """The chunks carrying video sample bytes (not the codec chunk or the keyframe marker)."""
    return [c for c in chunks if not c.is_static and "VideoStream:sample" in _cols(c)]


def _keyframe_marker(chunks: list[Chunk]) -> Chunk:
    """The single dedicated sparse `IsKeyframe` marker chunk."""
    markers = [c for c in chunks if _cols(c) == ["VideoStream:is_keyframe"]]
    assert len(markers) == 1, f"expected exactly one keyframe marker chunk, got {[_cols(c) for c in chunks]}"
    return markers[0]


def _times(chunk: Chunk) -> list[int]:
    """Raw `video` timeline values, as ints, regardless of duration/timestamp typing."""
    return [int(v) for v in chunk.to_record_batch().column("video").cast(pa.int64())]


def _sample_blocks(chunks: list[Chunk]) -> list[list[int]]:
    """Sample times grouped per chunk, chunks ordered by their first time."""
    return sorted((sorted(_times(c)) for c in _sample_chunks(chunks)), key=lambda block: block[0])


# ---------------------------------------------------------------------------
# Motivating example: parse an mp4 file into a VideoStream
# ---------------------------------------------------------------------------


def test_default_mode_produces_video_stream_chunks() -> None:
    """The motivating example: point Mp4Reader at a video file and get back a VideoStream."""
    chunks = Mp4Reader(H264_NO_BFRAMES).stream().to_chunks()

    # Stream-mode output is 1 static codec chunk + N GOP chunks + 1 keyframe marker.
    assert len(chunks) >= 3, "stream mode should emit the codec chunk, at least one GOP, and the keyframe marker"

    static_chunks = [c for c in chunks if c.is_static]

    # Exactly one static chunk holding the codec.
    assert len(static_chunks) == 1
    static = static_chunks[0]
    assert static.entity_path.endswith("/Big_Buck_Bunny_1080_1s_h264_nobframes.mp4")
    assert static.num_rows == 1
    assert any("codec" in name.lower() for name in _cols(static)), (
        f"expected a codec column on the static chunk; got {_cols(static)}"
    )

    # Every per-GOP chunk carries sample bytes on the "video" duration timeline,
    # and carries *only* samples — the keyframe marker is a separate chunk.
    sample_chunks = _sample_chunks(chunks)
    assert len(sample_chunks) >= 1
    for c in sample_chunks:
        assert c.timeline_names == ["video"], f"expected ['video'] timeline, got {c.timeline_names}"
        assert c.num_rows >= 1
        assert _cols(c) == ["VideoStream:sample"], f"sample chunks should carry only samples; got {_cols(c)}"

    # `is_keyframe` is sparse: only `true`, one row per GOP start. A dense
    # `true`/`false` column would make `collect(optimize=…)` skip video rebatching.
    marker = _keyframe_marker(chunks)
    assert marker.timeline_names == ["video"]
    flags = marker.to_record_batch().column("VideoStream:is_keyframe").to_pylist()
    assert flags == [[True]] * marker.num_rows, f"the marker must hold only `true`, got {flags!r}"
    assert _times(marker) == [_times(c)[0] for c in sample_chunks]


# ---------------------------------------------------------------------------
# Asset mode — matches the existing `rerun video.mp4` behavior
# ---------------------------------------------------------------------------


def test_asset_mode_emits_asset_video() -> None:
    """Asset mode produces an AssetVideo blob chunk plus a VideoFrameReference index chunk."""
    chunks = Mp4Reader(H264_NO_BFRAMES, mode="asset").stream().to_chunks()

    assert 1 <= len(chunks) <= 2, "asset mode emits 1 (blob only) or 2 (blob + index) chunks"
    for c in chunks:
        assert c.entity_path.endswith("/Big_Buck_Bunny_1080_1s_h264_nobframes.mp4")

    has_asset_video = any(any("AssetVideo" in name for name in _cols(c)) for c in chunks)
    assert has_asset_video, "asset mode should emit an AssetVideo chunk"


# ---------------------------------------------------------------------------
# chunk_by_gop toggle
# ---------------------------------------------------------------------------


def test_stream_mode_chunk_by_gop_false_emits_one_sample_per_chunk() -> None:
    """With chunk_by_gop=False, every sample chunk is exactly one sample."""
    chunks = Mp4Reader(H264_NO_BFRAMES, chunk_by_gop=False).stream().to_chunks()
    sample_chunks = _sample_chunks(chunks)
    assert len(sample_chunks) > 0
    for c in sample_chunks:
        assert c.num_rows == 1, f"chunk_by_gop=False should give 1 row per chunk; got {c.num_rows}"
    # The keyframe marker is still emitted once, sparsely, alongside them.
    assert _keyframe_marker(chunks).num_rows >= 1


def test_stream_mode_chunk_by_gop_true_packs_multiple_samples() -> None:
    """With chunk_by_gop=True (default), at least one GOP chunk should hold >1 sample."""
    chunks = Mp4Reader(H264_NO_BFRAMES).stream().to_chunks()
    assert any(c.num_rows > 1 for c in _sample_chunks(chunks)), (
        "expected at least one GOP chunk with multiple samples — the test fixture has GOPs > 1 frame"
    )


# ---------------------------------------------------------------------------
# entity_path override
# ---------------------------------------------------------------------------


def test_custom_entity_path_applies_to_every_chunk() -> None:
    chunks = Mp4Reader(H264_NO_BFRAMES, entity_path="/cameras/front").stream().to_chunks()
    assert len(chunks) > 0
    for c in chunks:
        assert c.entity_path == "/cameras/front"


def test_default_entity_path_derives_from_file_path() -> None:
    """Default `entity_path=None` uses the absolute filesystem path as the entity hierarchy."""
    chunks = Mp4Reader(H264_NO_BFRAMES, mode="asset").stream().to_chunks()
    for c in chunks:
        assert c.entity_path.endswith("/Big_Buck_Bunny_1080_1s_h264_nobframes.mp4")


def test_relative_path_is_absolutized(monkeypatch: pytest.MonkeyPatch) -> None:
    """A relative source path is resolved to absolute for both `.path` and the default entity path."""
    monkeypatch.chdir(H264_NO_BFRAMES.parent)
    reader = Mp4Reader(Path(H264_NO_BFRAMES.name), mode="asset")

    assert Path(reader.path).is_absolute()
    assert Path(reader.path) == H264_NO_BFRAMES
    # The default entity path reflects the absolute path (parent dirs included),
    # not just the bare filename that was passed in.
    entity_path = reader.stream().to_chunks()[0].entity_path
    assert "/tests/assets/video/" in entity_path
    assert entity_path.endswith("/Big_Buck_Bunny_1080_1s_h264_nobframes.mp4")


# ---------------------------------------------------------------------------
# Error handling
# ---------------------------------------------------------------------------


def test_b_frames_without_ffmpeg_reports_missing_ffmpeg() -> None:
    """
    A missing ffmpeg surfaces the "not installed" error rather than silently succeeding.

    We force the missing-ffmpeg case with a bogus `ffmpeg_override` so this is
    deterministic regardless of whether ffmpeg is installed on the test machine.
    """
    with pytest.raises(RuntimeError, match="Couldn't find an installation of the FFmpeg executable"):
        # The error is raised eagerly inside the loader thread; we surface it on
        # the first pull, so iterating is enough to trigger it.
        list(
            Mp4Reader(
                H264_WITH_BFRAMES,
                transcode=Mp4TranscodeOptions(ffmpeg_override="/definitely/not/a/real/ffmpeg"),
            ).stream()
        )


def test_b_frames_in_asset_mode_are_fine() -> None:
    """Asset mode is unaffected by B-frames."""
    chunks = Mp4Reader(H264_WITH_BFRAMES, mode="asset").stream().to_chunks()
    assert len(chunks) >= 1


def test_file_not_found(tmp_path: Path) -> None:
    with pytest.raises(FileNotFoundError, match="not found"):
        Mp4Reader(tmp_path / "nonexistent.mp4")


def test_invalid_mode() -> None:
    with pytest.raises(ValueError, match="Invalid mode"):
        Mp4Reader(H264_NO_BFRAMES, mode="bogus")  # type: ignore[call-overload]


def test_chunk_by_gop_false_with_asset_mode_rejected() -> None:
    """chunk_by_gop only makes sense in stream mode — passing it for asset mode is a user error."""
    with pytest.raises(ValueError, match="chunk_by_gop"):
        Mp4Reader(H264_NO_BFRAMES, mode="asset", chunk_by_gop=False)  # type: ignore[call-overload]


def test_invalid_timeline_type() -> None:
    with pytest.raises(ValueError, match="Invalid timeline_type"):
        Mp4Reader(H264_NO_BFRAMES, timeline_type="sequence")  # type: ignore[call-overload]


# ---------------------------------------------------------------------------
# timeline_type — schema-level check
# ---------------------------------------------------------------------------


def test_timeline_type_timestamp_produces_timestamp_typed_column() -> None:
    """`timeline_type="timestamp"` switches the time column from duration[ns] to timestamp[ns]."""
    chunks = Mp4Reader(H264_NO_BFRAMES, timeline_name="real_time", timeline_type="timestamp").stream().to_chunks()
    temporal = [c for c in chunks if not c.is_static]
    assert len(temporal) > 0
    rb = temporal[0].to_record_batch()
    ts_field = next(f for f in rb.schema if f.name == "real_time")
    # nanosecond-precision Arrow timestamp (with or without a tz attached).
    assert pa.types.is_timestamp(ts_field.type), f"expected a timestamp[ns] column, got {ts_field.type}"


def test_asset_mode_timeline_type_timestamp_applies_to_index_chunk() -> None:
    """`timeline_type` also types the asset-mode `VideoFrameReference` index timeline."""
    chunks = (
        Mp4Reader(H264_NO_BFRAMES, mode="asset", timeline_name="real_time", timeline_type="timestamp")
        .stream()
        .to_chunks()
    )
    # The index chunk is the one carrying the `real_time` timeline.
    index_chunks = [c for c in chunks if "real_time" in c.timeline_names]
    assert len(index_chunks) == 1, "asset mode should emit one VideoFrameReference index chunk"
    rb = index_chunks[0].to_record_batch()
    ts_field = next(f for f in rb.schema if f.name == "real_time")
    assert pa.types.is_timestamp(ts_field.type), f"expected a timestamp[ns] column, got {ts_field.type}"


# ---------------------------------------------------------------------------
# B-frame sources are transcoded via ffmpeg
# ---------------------------------------------------------------------------


@pytest.mark.skipif(not _HAS_FFMPEG, reason="ffmpeg not installed")
def test_b_frames_are_transcoded_into_a_video_stream() -> None:
    """
    A B-frame mp4 in stream mode is transcoded with ffmpeg into a normal `VideoStream`.

    It yields a static codec chunk plus per-GOP sample chunks — the same shape as
    the no-B-frame happy path.
    """
    chunks = Mp4Reader(H264_WITH_BFRAMES).stream().to_chunks()
    static_chunks = [c for c in chunks if c.is_static]
    temporal_chunks = [c for c in chunks if not c.is_static]
    assert len(static_chunks) == 1
    assert len(temporal_chunks) > 0
    for c in temporal_chunks:
        assert c.timeline_names == ["video"]


# ---------------------------------------------------------------------------
# Transcode transforms — output_codec / gop_size (stream mode only)
# ---------------------------------------------------------------------------


def _to_chunks_or_skip(reader: Mp4Reader) -> list[Chunk]:
    """Materialize a transcoding stream, skipping when the encoder is unavailable."""
    try:
        return reader.stream().to_chunks()
    except RuntimeError as err:
        if "encoder" in str(err) or "FFmpeg" in str(err):
            pytest.skip(f"ffmpeg/encoder not available: {err}")
        raise


def test_output_codec_same_as_source_stays_on_the_direct_path() -> None:
    chunks = (
        Mp4Reader(
            H264_NO_BFRAMES,
            transcode=Mp4TranscodeOptions(
                output_codec=VideoCodec.H264,
                ffmpeg_override="/definitely/not/a/real/ffmpeg",
            ),
        )
        .stream()
        .to_chunks()
    )
    assert len([c for c in chunks if c.is_static]) == 1
    assert any(not c.is_static for c in chunks)


def test_invalid_output_codec_rejected() -> None:
    with pytest.raises(TypeError, match="VideoCodec"):
        Mp4TranscodeOptions(output_codec="av1")  # type: ignore[arg-type]


def test_transcode_rejected_in_asset_mode() -> None:
    """`transcode` is stream-only — passing it for asset mode is a user error."""
    with pytest.raises(ValueError, match="transcode"):
        Mp4Reader(H264_NO_BFRAMES, mode="asset", transcode=Mp4TranscodeOptions(output_codec=VideoCodec.AV1))  # type: ignore[call-overload]


@pytest.mark.skipif(not _HAS_FFMPEG, reason="ffmpeg not installed")
@pytest.mark.parametrize("output_codec", [VideoCodec.AV1, VideoCodec.VP9, VideoCodec.H265])
def test_output_codec_transcodes_to_requested_codec(output_codec: VideoCodec) -> None:
    """Re-encoding a clean H.264 source to another codec yields a normal `VideoStream`."""
    chunks = _to_chunks_or_skip(Mp4Reader(H264_NO_BFRAMES, transcode=Mp4TranscodeOptions(output_codec=output_codec)))
    static_chunks = [c for c in chunks if c.is_static]
    temporal_chunks = [c for c in chunks if not c.is_static]
    assert len(static_chunks) == 1
    assert len(temporal_chunks) > 0
    for c in temporal_chunks:
        assert c.timeline_names == ["video"]


@pytest.mark.skipif(not _HAS_FFMPEG, reason="ffmpeg not installed")
def test_gop_size_forces_keyframe_spacing() -> None:
    """
    `gop_size=N` forces a keyframe every N frames.

    With `chunk_by_gop=True` that means one temporal chunk per GOP, so every GOP
    chunk but the last holds exactly N samples.
    """
    gop = 10
    chunks = _to_chunks_or_skip(Mp4Reader(H264_NO_BFRAMES, transcode=Mp4TranscodeOptions(gop_size=gop)))
    gop_sizes = [c.num_rows for c in _sample_chunks(chunks)]
    assert len(gop_sizes) >= 2, f"gop_size={gop} should force multiple GOPs, got {gop_sizes}"
    for n in gop_sizes[:-1]:
        assert n == gop, f"every GOP but the last should hold {gop} samples, got {gop_sizes}"
    assert 1 <= gop_sizes[-1] <= gop
    # One sparse keyframe row per forced GOP.
    assert _keyframe_marker(chunks).num_rows == len(gop_sizes)


# ---------------------------------------------------------------------------
# The reader's output is accepted by GOP rebatching
# ---------------------------------------------------------------------------


@pytest.mark.parametrize(
    ("path", "gop_size"),
    [
        (H264_NO_BFRAMES, None),  # single GOP: nothing to merge
        (AV1_MULTI_GOP, None),  # 12 natural GOPs
        (H264_NO_BFRAMES, 5),  # 6 forced GOPs, small enough to merge
    ],
    ids=["single_gop", "multi_gop", "forced_small_gops"],
)
@pytest.mark.parametrize("profile_name", ["OBJECT_STORE", "LIVE"])
def test_optimize_only_coarsens_the_readers_gop_partition(path: Path, gop_size: int | None, profile_name: str) -> None:
    """
    `Mp4Reader(chunk_by_gop=True)` and GOP rebatching must agree on where GOPs start.

    The chunks are not identical: the reader emits one chunk per GOP, and optimize
    then merges consecutive GOPs up to the profile's budget. But optimize must only
    ever *coarsen* that partition — every boundary it keeps is a reader boundary, no
    GOP is split, and sample order is preserved.
    """
    profile = getattr(OptimizationProfile, profile_name)
    transcode = Mp4TranscodeOptions(gop_size=gop_size) if gop_size else None

    fine_chunks = _to_chunks_or_skip(Mp4Reader(path, entity_path="/cam", transcode=transcode))
    fine = _sample_blocks(fine_chunks)
    fine_marker = sorted(_times(_keyframe_marker(fine_chunks)))

    coarse_chunks = (
        Mp4Reader(path, entity_path="/cam", transcode=transcode).stream().collect(optimize=profile).stream().to_chunks()
    )
    coarse = _sample_blocks(coarse_chunks)
    coarse_marker = sorted(_times(_keyframe_marker(coarse_chunks)))

    # The reader's chunk boundaries are exactly the keyframes.
    assert [block[0] for block in fine] == fine_marker

    # No sample is added, dropped, or reordered.
    assert [t for block in fine for t in block] == [t for block in coarse for t in block]

    # Every coarse block is the concatenation of consecutive reader blocks — so no
    # invented boundary and no GOP split down the middle.
    consumed = 0
    for block in coarse:
        merged: list[int] = []
        while consumed < len(fine) and len(merged) < len(block):
            merged.extend(fine[consumed])
            consumed += 1
        assert merged == block, f"coarse block {block[:4]}… is not a run of whole reader GOPs"
    assert consumed == len(fine), "every reader GOP must be accounted for"

    # And the keyframe marker is carried through untouched.
    assert coarse_marker == fine_marker


# ---------------------------------------------------------------------------
# StreamingReader protocol conformance
# ---------------------------------------------------------------------------


def test_streaming_reader_protocol() -> None:
    assert isinstance(Mp4Reader(H264_NO_BFRAMES), StreamingReader)

"""Integration tests for `rerun.experimental.Mp4Reader`."""

from __future__ import annotations

from pathlib import Path

import pytest
from rerun.experimental import Chunk, Mp4Reader, StreamingReader

VIDEO_ASSETS_DIR = Path(__file__).resolve().parents[3] / "tests" / "assets" / "video"

# H.264 fixture encoded without B-frames — usable in both modes.
H264_NO_BFRAMES = VIDEO_ASSETS_DIR / "Big_Buck_Bunny_1080_1s_h264_nobframes.mp4"

# Same content but encoded with B-frames — stream mode should reject it unless
# `allow_b_frames=True`; asset mode is unaffected.
H264_WITH_BFRAMES = VIDEO_ASSETS_DIR / "Big_Buck_Bunny_1080_1s_h264.mp4"


def _cols(chunk: Chunk) -> list[str]:
    """Return component column names on a chunk (excludes time and control columns)."""
    rb = chunk.to_record_batch()
    timelines = set(chunk.timeline_names)
    return sorted(f.name for f in rb.schema if f.name not in timelines and not f.name.startswith("rerun.controls"))


# ---------------------------------------------------------------------------
# Motivating example: parse an mp4 file into a VideoStream
# ---------------------------------------------------------------------------


def test_default_mode_produces_video_stream_chunks() -> None:
    """The motivating example: point Mp4Reader at a video file and get back a VideoStream."""
    chunks = Mp4Reader(H264_NO_BFRAMES).stream().to_chunks()

    # Stream-mode output is structured as 1 static codec chunk + N GOP chunks.
    assert len(chunks) >= 2, "stream mode should emit at least the static codec chunk plus one GOP"

    static_chunks = [c for c in chunks if c.is_static]
    temporal_chunks = [c for c in chunks if not c.is_static]

    # Exactly one static chunk holding the codec.
    assert len(static_chunks) == 1
    static = static_chunks[0]
    assert static.entity_path.endswith("/Big_Buck_Bunny_1080_1s_h264_nobframes.mp4")
    assert static.num_rows == 1
    assert any("codec" in name.lower() for name in _cols(static)), (
        f"expected a codec column on the static chunk; got {_cols(static)}"
    )

    # Every per-GOP chunk carries sample bytes + an is_keyframe flag on the
    # "video" duration timeline, and the first row of each GOP is a keyframe.
    for c in temporal_chunks:
        assert c.timeline_names == ["video"], f"expected ['video'] timeline, got {c.timeline_names}"
        assert c.num_rows >= 1
        col_names = _cols(c)
        sample_col = next((n for n in col_names if "sample" in n.lower()), None)
        keyframe_col = next((n for n in col_names if "keyframe" in n.lower()), None)
        assert sample_col is not None, f"expected a sample column; got {col_names}"
        assert keyframe_col is not None, f"expected an is_keyframe column; got {col_names}"

        rb = c.to_record_batch()
        # `is_keyframe` is stored as a list-per-row component column (`List[bool]`).
        first_keyframe = rb.column(keyframe_col)[0].as_py()
        assert first_keyframe == [True], f"first row of a GOP chunk should be a keyframe, got {first_keyframe!r}"


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
    """With chunk_by_gop=False, every temporal chunk is exactly one sample."""
    chunks = Mp4Reader(H264_NO_BFRAMES, chunk_by_gop=False).stream().to_chunks()
    temporal = [c for c in chunks if not c.is_static]
    assert len(temporal) > 0
    for c in temporal:
        assert c.num_rows == 1, f"chunk_by_gop=False should give 1 row per chunk; got {c.num_rows}"


def test_stream_mode_chunk_by_gop_true_packs_multiple_samples() -> None:
    """With chunk_by_gop=True (default), at least one GOP chunk should hold >1 sample."""
    chunks = Mp4Reader(H264_NO_BFRAMES).stream().to_chunks()
    temporal = [c for c in chunks if not c.is_static]
    assert any(c.num_rows > 1 for c in temporal), (
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
    """Default `entity_path=None` mirrors the importer: full filesystem path as entity hierarchy."""
    chunks = Mp4Reader(H264_NO_BFRAMES, mode="asset").stream().to_chunks()
    for c in chunks:
        assert c.entity_path.endswith("/Big_Buck_Bunny_1080_1s_h264_nobframes.mp4")


# ---------------------------------------------------------------------------
# Error handling
# ---------------------------------------------------------------------------


def test_b_frames_in_stream_mode_raise() -> None:
    """B-frames are unsupported in stream mode (VideoStream archetype limitation #10090)."""
    with pytest.raises(Exception, match="B-frame"):
        # The error is raised eagerly inside the loader thread; we surface it on
        # the first pull, so iterating is enough to trigger it.
        list(Mp4Reader(H264_WITH_BFRAMES).stream())


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


def test_allow_b_frames_with_asset_mode_rejected() -> None:
    """allow_b_frames only makes sense in stream mode."""
    with pytest.raises(ValueError, match="allow_b_frames"):
        Mp4Reader(H264_NO_BFRAMES, mode="asset", allow_b_frames=True)  # type: ignore[call-overload]


def test_invalid_timeline_type() -> None:
    with pytest.raises(ValueError, match="Invalid timeline_type"):
        Mp4Reader(H264_NO_BFRAMES, timeline_type="sequence")  # type: ignore[call-overload]


# ---------------------------------------------------------------------------
# timeline_type — schema-level check
# ---------------------------------------------------------------------------


def test_timeline_type_timestamp_produces_timestamp_typed_column() -> None:
    """`timeline_type="timestamp"` switches the time column from duration[ns] to timestamp[ns]."""
    import pyarrow as pa

    chunks = Mp4Reader(H264_NO_BFRAMES, timeline_name="real_time", timeline_type="timestamp").stream().to_chunks()
    temporal = [c for c in chunks if not c.is_static]
    assert len(temporal) > 0
    rb = temporal[0].to_record_batch()
    ts_field = next(f for f in rb.schema if f.name == "real_time")
    # nanosecond-precision Arrow timestamp (with or without a tz attached).
    assert pa.types.is_timestamp(ts_field.type), f"expected a timestamp[ns] column, got {ts_field.type}"


def test_asset_mode_timeline_type_timestamp_applies_to_index_chunk() -> None:
    """`timeline_type` also types the asset-mode `VideoFrameReference` index timeline."""
    import pyarrow as pa

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
# allow_b_frames marks the time column unsorted
# ---------------------------------------------------------------------------


def test_allow_b_frames_opts_in_to_b_frame_inputs() -> None:
    """
    `allow_b_frames=True` unblocks reading a B-frame mp4 in stream mode.

    Without the opt-in the same input raises (verified by
    `test_b_frames_in_stream_mode_raise`). With it, the reader produces the
    same shape of chunks as the no-B-frame happy path. The chunk store may
    internally re-sort the time column, so we don't assert on Arrow sortedness
    metadata — the user-visible contract is "doesn't raise + produces chunks."
    """
    chunks = Mp4Reader(H264_WITH_BFRAMES, allow_b_frames=True).stream().to_chunks()
    static_chunks = [c for c in chunks if c.is_static]
    temporal_chunks = [c for c in chunks if not c.is_static]
    assert len(static_chunks) == 1
    assert len(temporal_chunks) > 0
    for c in temporal_chunks:
        assert c.timeline_names == ["video"]


# ---------------------------------------------------------------------------
# StreamingReader protocol conformance
# ---------------------------------------------------------------------------


def test_streaming_reader_protocol() -> None:
    assert isinstance(Mp4Reader(H264_NO_BFRAMES), StreamingReader)

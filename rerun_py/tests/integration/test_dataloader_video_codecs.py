"""
Integration tests for various video decoding scenarios seen in the video decoder.

It tests each codec with a built-in keyframe detector (`h264`, `h265`, `av1`) at several GOP lengths, against both decode paths.
"""

from __future__ import annotations

import multiprocessing
from dataclasses import dataclass
from typing import TYPE_CHECKING, Literal

import av
import numpy as np
import pyarrow as pa
import pytest
import rerun as rr
import torch
from av.bitstream import BitStreamFilterContext
from rerun.experimental.dataloader import (
    DataSource,
    Field,
    FixedRateSampling,
    NumericDecoder,
    RerunMapDataset,
    VideoFrameDecoder,
)

if TYPE_CHECKING:
    from pathlib import Path

# `RerunMapDataset.__init__` warns when the default start method is `fork`,
# because forked DataLoader workers would deadlock on their first catalog call.
# These tests never spawn workers (`num_workers=0`), but the warning fires at
# construction time. Switching the start method to `spawn` is what the warning
# asks for and removes the noise at its source.
if multiprocessing.get_start_method(allow_none=True) is None:
    multiprocessing.set_start_method("spawn")


@dataclass(frozen=True)
class CodecConfig:
    """Everything the generator, decoder, and SDK need for one codec."""

    encoder: str
    """PyAV/ffmpeg encoder name."""

    annex_b_filter: str | None
    """Bitstream filter that converts demuxed packets to Annex B, or `None` to pass raw bytes."""

    sdk_codec: rr.VideoCodec
    """Codec enum logged on the `VideoStream` archetype."""

    decoder_codec: str
    """`codec=` string passed to `VideoFrameDecoder`."""

    force_no_b_frames: bool = False
    """If True, force `max_b_frames = 0` so DTS == PTS (required for `VideoStream`).

    Needed for H.264 and H.265 (libx264 and libx265 both emit reordered B-frames
    by default). AV1 never has DTS != PTS, so we leave the encoder default in
    place to exercise more realistic bitstreams.
    """


CODEC_CONFIGS = {
    "h264": CodecConfig("libx264", "h264_mp4toannexb", rr.VideoCodec.H264, "h264", force_no_b_frames=True),
    "h265": CodecConfig("libx265", "hevc_mp4toannexb", rr.VideoCodec.H265, "h265", force_no_b_frames=True),
    "av1": CodecConfig("libaom-av1", None, rr.VideoCodec.AV1, "av1"),
}

# 1 = every frame is a keyframe; 8 and 24 give multiple GOPs over NUM_FRAMES.
GOP_SIZES = [1, 8, 24]

NUM_FRAMES = 96
WIDTH = 64
HEIGHT = 128


def _encoder_available(name: str) -> bool:
    """True if this PyAV build can encode with *name*."""
    try:
        av.codec.Codec(name, "w")
    except Exception:
        return False
    return True


def _synthetic_frame(index: int) -> av.VideoFrame:
    """A small RGB frame with content that changes each index (so motion compensation has work to do)."""
    pixels = np.empty((HEIGHT, WIDTH, 3), dtype=np.uint8)
    pixels[:, :, 0] = ((np.arange(WIDTH) + index) % 256)[np.newaxis, :]
    pixels[:, :, 1] = ((np.arange(HEIGHT) + index) % 256)[:, np.newaxis]
    pixels[:, :, 2] = (index * 7) % 256
    return av.VideoFrame.from_ndarray(pixels, format="rgb24")


def _generate_stream(
    tmp_path: Path, config: CodecConfig, gop_size: int, num_frames: int = NUM_FRAMES
) -> tuple[list[bytes], list[int]]:
    """
    Encode a synthetic clip with a fixed keyframe cadence, then demux it back to per-frame samples.

    Returns `(samples, keyframe_indices)`: one encoded sample per frame, and
    the indices into `samples` that are codec keyframes.
    """
    tmp_path.mkdir(parents=True, exist_ok=True)
    container_path = tmp_path / "source.mp4"

    output = av.open(str(container_path), "w")
    try:
        stream = output.add_stream(config.encoder, rate=30)
        assert isinstance(stream, av.VideoStream)
        stream.width = WIDTH
        stream.height = HEIGHT
        stream.pix_fmt = "yuv420p"
        stream.gop_size = gop_size
        if config.force_no_b_frames:
            stream.max_b_frames = 0  # Keep DTS == PTS, required for VideoStream.

        # Pin both max and min keyframe interval to `gop_size` so keyframes land on a fixed cadence
        # (no early scene-cut keyframes shortening a GOP).
        stream.codec_context.options = {"g": str(gop_size), "keyint_min": str(gop_size)}

        for index in range(num_frames):
            for packet in stream.encode(_synthetic_frame(index)):
                output.mux(packet)
        for packet in stream.encode(None):
            output.mux(packet)
    finally:
        output.close()

    keyframe_indices: list[int] = []
    samples: list[bytes] = []

    container = av.open(str(container_path))
    try:
        video_stream = container.streams.video[0]
        bsf = None
        if config.annex_b_filter is not None:
            bsf = BitStreamFilterContext(config.annex_b_filter, video_stream)

        def collect_sample(packet: av.Packet) -> None:
            if packet.pts is None or packet.size == 0:
                return
            if packet.is_keyframe:
                keyframe_indices.append(len(samples))
            samples.append(bytes(packet))

        for packet in container.demux(video_stream):
            if bsf is None:
                collect_sample(packet)
            else:
                for filtered in bsf.filter(packet):
                    collect_sample(filtered)
    finally:
        container.close()

    assert keyframe_indices, "generated clip must contain at least one keyframe"
    assert keyframe_indices[0] == 0, "first packet of the generated clip must be a keyframe"

    return samples, keyframe_indices


KeyframeLogging = Literal["sparse", "dense", "none"]


def _build_rrd(
    rrd_path: Path,
    config: CodecConfig,
    samples: list[bytes],
    keyframe_indices: list[int],
    *,
    keyframe_logging: KeyframeLogging,
) -> None:
    """
    Log one `VideoStream` sample per frame, a companion scalar, and optionally the `is_keyframe` column.

    `keyframe_logging` controls how `is_keyframe` is populated:
    - `"sparse"`: only `True` at keyframe indices (relies on latest-at fill for non-keyframes).
    - `"dense"`: `True` at keyframes and `False` at every other frame (no latest-at fill needed,
       but exposes any decoder code that mistakenly treats `False` as "unknown").
    - `"none"`: don't log `is_keyframe`; decoder must fall back to the heuristic.
    """
    with rr.RecordingStream(
        "rerun_example_test_dataloader_video_codecs", recording_id="dataloader-video-codecs"
    ) as rec:
        rec.save(rrd_path)
        rec.log("/video", rr.VideoStream(codec=config.sdk_codec), static=True)
        rec.send_columns(
            "/video",
            indexes=[rr.TimeColumn("frame", sequence=list(range(len(samples))))],
            columns=rr.VideoStream.columns(sample=samples),
        )
        # Scalar column so decoder queries must rely on the decode window, not just the target row.
        rec.send_columns(
            "/state",
            indexes=[rr.TimeColumn("frame", sequence=list(range(len(samples))))],
            columns=rr.Scalars.columns(scalars=[float(i) for i in range(len(samples))]),
        )
        if keyframe_logging == "sparse":
            rec.send_columns(
                "/video",
                indexes=[rr.TimeColumn("frame", sequence=keyframe_indices)],
                columns=rr.VideoStream.columns(is_keyframe=[True] * len(keyframe_indices)),
            )
        elif keyframe_logging == "dense":
            keyframe_set = set(keyframe_indices)
            flags = [index in keyframe_set for index in range(len(samples))]
            rec.send_columns(
                "/video",
                indexes=[rr.TimeColumn("frame", sequence=list(range(len(samples))))],
                columns=rr.VideoStream.columns(is_keyframe=flags),
            )


def _decode_targets(
    rrd_dir: Path, config: CodecConfig, keyframe_interval: int, targets: list[int]
) -> dict[int, dict[str, torch.Tensor | None]]:
    """Serve *rrd_dir* in-memory and decode each target index, returning `{target: sample}`."""
    results: dict[int, dict[str, torch.Tensor | None]] = {}
    with rr.server.Server(datasets={"video": rrd_dir}) as server:
        ds = server.client().get_dataset("video")
        source = DataSource(ds)
        dataset = RerunMapDataset(
            source,
            "frame",
            {
                "image": Field(
                    "/video:VideoStream:sample",
                    decode=VideoFrameDecoder(codec=config.decoder_codec, keyframe_interval=keyframe_interval),
                ),
                "state": Field("/state:Scalars:scalars", decode=NumericDecoder()),
            },
        )
        for target in targets:
            results[target] = dataset[target]
    return results


@pytest.mark.parametrize("codec", list(CODEC_CONFIGS))
@pytest.mark.parametrize("gop_size", GOP_SIZES)
@pytest.mark.parametrize(
    "keyframe_logging",
    ["sparse", "dense", "none"],
    ids=["anchor_sparse", "anchor_dense", "heuristic"],
)
def test_decode_matrix(tmp_path: Path, codec: str, gop_size: int, keyframe_logging: KeyframeLogging) -> None:
    """
    Decode the first frame, the last frame, and (when the GOP spans multiple frames) a mid-GOP frame for one (codec, gop, path) cell.

    The anchor paths (`sparse`/`dense`) use `keyframe_interval=1` so any mid-GOP
    target must consult the `is_keyframe` column. The `dense` variant logs an
    explicit `False` at every non-keyframe, which exercises the path where
    latest-at fill would otherwise propagate `False` into later rows. The
    `heuristic` path drops `is_keyframe` entirely and uses `keyframe_interval=gop_size`.
    """
    config = CODEC_CONFIGS[codec]
    if not _encoder_available(config.encoder):
        pytest.skip(f"PyAV build lacks the {config.encoder} encoder")

    samples, keyframe_indices = _generate_stream(tmp_path / "gen", config, gop_size)
    rrd_dir = tmp_path / "recording"
    rrd_dir.mkdir()
    _build_rrd(rrd_dir / "recording.rrd", config, samples, keyframe_indices, keyframe_logging=keyframe_logging)

    targets = [0, len(samples) - 1]
    # Pick a mid-GOP target strictly between the first two real keyframes, at least two frames
    # past keyframe[0] so the anchor case's window `[target - 1, target]` contains no keyframe
    # and the decode must go through the `is_keyframe` anchor instead of the heuristic.
    if gop_size > 1:
        assert len(keyframe_indices) >= 2, "need at least two keyframes to pick a mid-GOP target"
        mid_gop_target = (keyframe_indices[0] + keyframe_indices[1]) // 2
        assert mid_gop_target - keyframe_indices[0] >= 2
        assert mid_gop_target < keyframe_indices[1]
        targets.append(mid_gop_target)

    keyframe_interval = gop_size if keyframe_logging == "none" else 1
    results = _decode_targets(rrd_dir, config, keyframe_interval, targets)

    for target in targets:
        sample = results[target]
        image = sample["image"]
        assert image is not None, f"decode returned None for target {target}"
        assert image.ndim == 3
        assert image.shape[0] == 3  # (C, H, W)
        assert image.shape[1] == HEIGHT
        assert image.shape[2] == WIDTH
        state = sample["state"]
        assert state is not None
        assert float(state[0]) == float(target)


# ---------------------------------------------------------------------------
# Duplicate-sample handling.
#
# When frames are dropped, `fill_latest_at` backfills the empty grid slots with
# the previous frame's encoded bytes, so the decode window contains consecutive
# duplicate samples. Re-feeding a duplicate packet corrupts the decoder's
# reference state, so `VideoFrameDecoder` skips consecutive duplicates. The
# tests below pin that behavior; they fail if the dedup is dropped.
# ---------------------------------------------------------------------------

DEDUP_GOP_SIZE = 5  # Multiple GOPs over NUM_FRAMES, so a mid-GOP target has P-frames.


def _blob_column(samples: list[bytes]) -> pa.ChunkedArray:
    """Wrap encoded samples as the `list<binary>` column shape the decoder expects."""
    return pa.chunked_array([pa.array([[sample] for sample in samples], type=pa.list_(pa.binary()))])


def _decode_window(decoder: VideoFrameDecoder, samples: list[bytes], target: int) -> torch.Tensor | None:
    """Decode a window of encoded samples through the public `VideoFrameDecoder.decode`."""
    return decoder.decode(_blob_column(samples), target, "segment")


def test_duplicate_window_matches_clean_decode(tmp_path: Path) -> None:
    """
    A duplicated window decodes to the same frame as the clean window.

    Repeats one sample (as `fill_latest_at` would on an empty grid slot) and
    asserts the decode is unchanged, because the decoder drops the duplicate.
    """
    config = CODEC_CONFIGS["h264"]
    samples, keyframe_indices = _generate_stream(tmp_path / "gen", config, DEDUP_GOP_SIZE)

    keyframe = keyframe_indices[1]  # second GOP, so a P-frame references this keyframe
    target = keyframe + 2
    assert target not in keyframe_indices

    decoder = VideoFrameDecoder(codec=config.decoder_codec, keyframe_interval=len(samples))

    clean_window = samples[keyframe : target + 1]
    # Repeat the frame just before the target, exactly as `fill_latest_at` backfills an empty slot.
    duplicated_window = [*samples[keyframe:target], samples[target - 1], samples[target]]
    assert duplicated_window != clean_window, "the duplicated window must actually contain a repeat"

    clean = _decode_window(decoder, clean_window, target)
    duplicated = _decode_window(decoder, duplicated_window, target)

    assert clean is not None and duplicated is not None
    assert torch.equal(duplicated, clean), "duplicate samples in the window must not change the decoded frame"


def _build_timestamped_video_rrd(
    rrd_path: Path,
    config: CodecConfig,
    samples: list[bytes],
    keyframe_indices: list[int],
    timestamps_ns: list[int],
) -> None:
    """Log the VideoStream on a timestamp timeline at explicit per-frame timestamps, with sparse `is_keyframe`."""
    timestamps = np.array(timestamps_ns, dtype="datetime64[ns]")
    keyframe_timestamps = timestamps[keyframe_indices]

    with rr.RecordingStream("rerun_example_test_dataloader_video_dropped", recording_id="dropped-frames") as rec:
        rec.save(rrd_path)
        rec.log("/video", rr.VideoStream(codec=config.sdk_codec), static=True)
        rec.send_columns(
            "/video",
            indexes=[rr.TimeColumn("real_time", timestamp=timestamps)],
            columns=rr.VideoStream.columns(sample=samples),
        )
        rec.send_columns(
            "/video",
            indexes=[rr.TimeColumn("real_time", timestamp=keyframe_timestamps)],
            columns=rr.VideoStream.columns(is_keyframe=[True] * len(keyframe_indices)),
        )


def test_fixed_rate_sampling_duplicates_decode_correctly(tmp_path: Path) -> None:
    """
    Exercise the deployment path: dropped frames + `FixedRateSampling` + `fill_latest_at`.

    Real frames sit on a sparse subset of a 30 Hz grid, so the fixed-rate decode
    window for a mid-GOP target is backfilled with duplicate samples. The served
    decode matches a clean decode of the de-duplicated real frames.
    """
    config = CODEC_CONFIGS["h264"]
    samples, keyframe_indices = _generate_stream(tmp_path / "gen", config, DEDUP_GOP_SIZE)

    rate_hz = 30.0
    ns_per_slot = round(1e9 / rate_hz)

    # Target the second P-frame of the second GOP (`keyframe + 2`).
    # The grid slot just before it has no captured frame, so `fill_latest_at` backfills it with
    # the previous P-frame's bytes, the duplicate that desyncs libav.
    keyframe_real = keyframe_indices[1]
    target_real = keyframe_real + 2
    assert target_real not in keyframe_indices and target_real < keyframe_indices[2]

    slot_of_frame = list(range(len(samples)))
    for frame_index in range(target_real, len(samples)):
        slot_of_frame[frame_index] += 1  # leave the grid slot just before the target empty
    target_slot = slot_of_frame[target_real]

    rrd_dir = tmp_path / "recording"
    rrd_dir.mkdir()
    timestamps_ns = [slot * ns_per_slot for slot in slot_of_frame]
    _build_timestamped_video_rrd(rrd_dir / "recording.rrd", config, samples, keyframe_indices, timestamps_ns)

    # The real frames the grid maps to across the window, with the duplicate at the empty slot.
    keyframe_slot = slot_of_frame[keyframe_real]
    window_real_indices = [
        max(k for k, s in enumerate(slot_of_frame) if s <= grid_slot)
        for grid_slot in range(keyframe_slot, target_slot + 1)
    ]
    assert window_real_indices == [keyframe_real, keyframe_real + 1, keyframe_real + 1, target_real], (
        f"unexpected window layout {window_real_indices}"
    )

    # Ground truth: a clean decode of the de-duplicated real frames in the window.
    decoder = VideoFrameDecoder(codec=config.decoder_codec, keyframe_interval=len(samples), fps_estimate=rate_hz)
    clean_samples = samples[keyframe_real : target_real + 1]
    ground_truth = _decode_window(decoder, clean_samples, target_slot)
    assert ground_truth is not None

    with rr.server.Server(datasets={"video": rrd_dir}) as server:
        ds = server.client().get_dataset("video")
        dataset = RerunMapDataset(
            DataSource(ds),
            "real_time",
            {
                "image": Field(
                    "/video:VideoStream:sample",
                    decode=VideoFrameDecoder(
                        codec=config.decoder_codec, keyframe_interval=len(samples), fps_estimate=rate_hz
                    ),
                ),
            },
            timeline_sampling=FixedRateSampling(rate_hz=rate_hz),
        )
        served = dataset[target_slot]["image"]

    assert served is not None, "served decode unexpectedly returned None"
    assert torch.equal(served, ground_truth), "fixed-rate duplicate samples must not change the decoded frame"


OFF_GRID_NUM_FRAMES = 96
OFF_GRID_GOP_SIZE = 5
OFF_GRID_REAL_RATE_HZ = 27.0
OFF_GRID_GRID_RATE_HZ = 30.0


def test_off_grid_capture_rate_decodes_correctly(tmp_path: Path) -> None:
    """
    Every grid slot of a ~27 fps capture decodes to the de-duplicated real frames up to that slot.

    A 30 fps camera dropping frames captures below nominal; ~27 fps served on the 30 Hz grid means every
    slot is misaligned and the grid periodically laps the capture, backfilling duplicate samples.
    """
    config = CODEC_CONFIGS["h264"]
    if not _encoder_available(config.encoder):
        pytest.skip(f"PyAV build lacks the {config.encoder} encoder")

    samples, keyframe_indices = _generate_stream(
        tmp_path / "gen", config, OFF_GRID_GOP_SIZE, num_frames=OFF_GRID_NUM_FRAMES
    )

    ns_per_slot = round(1e9 / OFF_GRID_GRID_RATE_HZ)
    timestamps_ns = [round(i / OFF_GRID_REAL_RATE_HZ * 1e9) for i in range(len(samples))]

    rrd_dir = tmp_path / "recording"
    rrd_dir.mkdir()
    _build_timestamped_video_rrd(rrd_dir / "recording.rrd", config, samples, keyframe_indices, timestamps_ns)

    # Resolve each grid slot to the real frame `fill_latest_at` backfills it with
    # (latest real frame at or before the slot) and that frame's prior keyframe.
    timestamps_array = np.array(timestamps_ns)
    keyframe_array = np.array(keyframe_indices)
    num_slots = (timestamps_ns[-1] - timestamps_ns[0]) // ns_per_slot + 1
    real_for_slot = [
        int(np.searchsorted(timestamps_array, timestamps_ns[0] + slot * ns_per_slot, side="right") - 1)
        for slot in range(num_slots)
    ]
    prior_keyframe_real = [
        int(keyframe_array[np.searchsorted(keyframe_array, real, side="right") - 1]) for real in real_for_slot
    ]

    duplicate_slots = [slot for slot in range(1, num_slots) if real_for_slot[slot] == real_for_slot[slot - 1]]
    assert duplicate_slots, "off-grid capture must lap the grid and produce at least one duplicate slot"

    # Ground truth: a clean decode of the de-duplicated real frames for each slot.
    decoder = VideoFrameDecoder(
        codec=config.decoder_codec, keyframe_interval=len(samples), fps_estimate=OFF_GRID_GRID_RATE_HZ
    )
    ground_truth = []
    for slot in range(num_slots):
        clean_samples = samples[prior_keyframe_real[slot] : real_for_slot[slot] + 1]
        decoded = _decode_window(decoder, clean_samples, slot)
        assert decoded is not None, f"clean decode returned None for slot {slot}"
        ground_truth.append(decoded)

    with rr.server.Server(datasets={"video": rrd_dir}) as server:
        ds = server.client().get_dataset("video")
        dataset = RerunMapDataset(
            DataSource(ds),
            "real_time",
            {
                "image": Field(
                    "/video:VideoStream:sample",
                    decode=VideoFrameDecoder(
                        codec=config.decoder_codec, keyframe_interval=len(samples), fps_estimate=OFF_GRID_GRID_RATE_HZ
                    ),
                ),
            },
            timeline_sampling=FixedRateSampling(rate_hz=OFF_GRID_GRID_RATE_HZ),
        )
        assert len(dataset) == num_slots, f"expected {num_slots} grid slots, got {len(dataset)}"
        served = dataset.__getitems__(list(range(num_slots)))  # one batched query for the whole grid

    for slot in range(num_slots):
        image = served[slot]["image"]
        assert image is not None, f"served decode returned None for slot {slot}"
        assert torch.equal(image, ground_truth[slot]), f"off-grid decode mismatch at slot {slot}"

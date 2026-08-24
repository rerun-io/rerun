"""
Integration tests for the keyframe-aware video dataloader.

Exercises `RerunMapDataset` + `VideoFrameDecoder` end-to-end against a small
H.264 stream served via `rr.server.Server`, including validation of the
required sibling `is_keyframe` column.
"""

from __future__ import annotations

import pathlib
from typing import TYPE_CHECKING, Literal

import pytest
import rerun as rr
import torch
from rerun.experimental.dataloader import (
    DataSource,
    Field,
    NumericDecoder,
    RerunMapDataset,
    VideoFrameDecoder,
    Yuv420Frame,
)

if TYPE_CHECKING:
    from pathlib import Path


VIDEO_ASSET = (
    pathlib.Path(__file__).parents[3] / "tests" / "assets" / "video" / "Big_Buck_Bunny_1080_1s_h264_nobframes.mp4"
)


def _build_h264_rrd(rrd_path: Path, *, log_is_keyframe: bool) -> list[int]:
    """
    Build an RRD with one VideoStream sample per demuxed packet on a sequence timeline.

    Returns the list of frame indices that are codec keyframes. When
    `log_is_keyframe` is true, also writes a sparse `is_keyframe=True` row on
    those indices.
    """
    import av
    from av.bitstream import BitStreamFilterContext

    container = av.open(str(VIDEO_ASSET))
    keyframe_indices: list[int] = []
    samples: list[bytes] = []
    try:
        video_stream = container.streams.video[0]
        bsf = BitStreamFilterContext("h264_mp4toannexb", video_stream)

        def absorb(packet: av.Packet) -> None:
            if packet.pts is None or packet.size == 0:
                return
            if packet.is_keyframe:
                keyframe_indices.append(len(samples))
            samples.append(bytes(packet))

        for packet in container.demux(video_stream):
            for out in bsf.filter(packet):
                absorb(out)
    finally:
        container.close()

    assert keyframe_indices, "test asset must contain at least one keyframe"
    assert keyframe_indices[0] == 0, "test asset's first packet must be a keyframe"

    with rr.RecordingStream("rerun_example_test_dataloader_video", recording_id="dataloader-video") as rec:
        rec.save(rrd_path)
        rec.log("/video", rr.VideoStream(codec=rr.VideoCodec.H264), static=True)
        rec.send_columns(
            "/video",
            indexes=[rr.TimeColumn("frame", sequence=list(range(len(samples))))],
            columns=rr.VideoStream.columns(sample=samples),
        )
        # Companion scalar so tests cover the mixed-decoder query path
        # (`prior_keyframe_path` on a non-video decoder must return None, not raise).
        rec.send_columns(
            "/state",
            indexes=[rr.TimeColumn("frame", sequence=list(range(len(samples))))],
            columns=rr.Scalars.columns(scalars=[float(i) for i in range(len(samples))]),
        )
        if log_is_keyframe:
            rec.send_columns(
                "/video",
                indexes=[rr.TimeColumn("frame", sequence=keyframe_indices)],
                columns=rr.VideoStream.columns(is_keyframe=[True] * len(keyframe_indices)),
            )

    return keyframe_indices


@pytest.fixture
def rrd_with_keyframes(tmp_path: Path) -> tuple[Path, list[int]]:
    rrd_dir = tmp_path / "with_keyframes"
    rrd_dir.mkdir()
    keyframes = _build_h264_rrd(rrd_dir / "recording.rrd", log_is_keyframe=True)
    return rrd_dir, keyframes


@pytest.fixture
def rrd_without_keyframes(tmp_path: Path) -> tuple[Path, list[int]]:
    rrd_dir = tmp_path / "without_keyframes"
    rrd_dir.mkdir()
    keyframes = _build_h264_rrd(rrd_dir / "recording.rrd", log_is_keyframe=False)
    return rrd_dir, keyframes


@pytest.mark.filterwarnings("ignore:The default multiprocessing start method is 'fork':RuntimeWarning")
def test_keyframe_path_decodes_mid_gop_target(rrd_with_keyframes: tuple[Path, list[int]]) -> None:
    """The `is_keyframe` column expands a mid-GOP request back to its prior keyframe."""
    rrd_dir, keyframes = rrd_with_keyframes
    target = keyframes[0] + 5
    assert target not in keyframes, "target must sit strictly between keyframes"

    with rr.server.Server(datasets={"video": rrd_dir}) as server:
        ds = server.client().get_dataset("video")
        source = DataSource(ds)
        dataset = RerunMapDataset(
            source,
            "frame",
            {
                "image": Field(
                    "/video:VideoStream:sample",
                    decode=VideoFrameDecoder(codec="h264"),
                ),
                "state": Field("/state:Scalars:scalars", decode=NumericDecoder()),
            },
        )
        sample = dataset[target]

    image = sample["image"]
    assert isinstance(image, torch.Tensor)
    assert image.ndim == 3
    assert image.shape[0] == 3  # (C, H, W)
    state = sample["state"]
    assert isinstance(state, torch.Tensor)
    assert float(state[0]) == float(target)


@pytest.mark.filterwarnings("ignore:The default multiprocessing start method is 'fork':RuntimeWarning")
@pytest.mark.parametrize("window_storage", ["copy", "view"])
def test_windowed_video_returns_frame_stack(
    rrd_with_keyframes: tuple[Path, list[int]],
    window_storage: Literal["copy", "view"],
) -> None:
    rrd_dir, keyframes = rrd_with_keyframes
    target = keyframes[0] + 5
    window = (-3, -2, -1, 0)

    with rr.server.Server(datasets={"video": rrd_dir}) as server:
        source = DataSource(server.client().get_dataset("video"))
        dataset = RerunMapDataset(
            source,
            "frame",
            {
                "clip": Field(
                    "/video:VideoStream:sample",
                    decode=VideoFrameDecoder(
                        codec="h264",
                        window_storage=window_storage,
                    ),
                    window=window,
                ),
                "single": Field(
                    "/video:VideoStream:sample",
                    decode=VideoFrameDecoder(codec="h264"),
                ),
            },
        )
        sample = dataset[target]

    clip = sample["clip"]
    frame = sample["single"]
    assert isinstance(clip, torch.Tensor)
    assert isinstance(frame, torch.Tensor)
    assert tuple(clip.shape[:2]) == (len(window), 3)
    assert clip.shape[2:] == frame.shape[1:]
    assert torch.equal(clip[-1], frame)


@pytest.mark.filterwarnings("ignore:The default multiprocessing start method is 'fork':RuntimeWarning")
def test_windowed_video_can_remain_yuv420(rrd_with_keyframes: tuple[Path, list[int]]) -> None:
    """The YUV output crosses the complete map-dataset pipeline without implicit RGB conversion."""
    rrd_dir, keyframes = rrd_with_keyframes
    target = keyframes[0] + 5
    window = (-3, -2, -1, 0)

    with rr.server.Server(datasets={"video": rrd_dir}) as server:
        source = DataSource(server.client().get_dataset("video"))
        dataset = RerunMapDataset(
            source,
            "frame",
            {
                "clip": Field(
                    "/video:VideoStream:sample",
                    decode=VideoFrameDecoder(codec="h264", window_storage="view", output_format="yuv420p"),
                    window=window,
                ),
            },
        )
        clip = dataset[target]["clip"]

    assert isinstance(clip, Yuv420Frame)
    assert clip.y.shape[:2] == (len(window), 1)
    assert clip.uv.shape[:2] == (len(window), 2)
    assert clip.y.shape[-2] == clip.uv.shape[-2] * 2
    assert clip.y.shape[-1] == clip.uv.shape[-1] * 2


@pytest.mark.filterwarnings("ignore:The default multiprocessing start method is 'fork':RuntimeWarning")
def test_missing_is_keyframe_column_raises(
    rrd_without_keyframes: tuple[Path, list[int]],
) -> None:
    """A video field without its sibling `is_keyframe` column is rejected before fetching samples."""
    rrd_dir, keyframes = rrd_without_keyframes
    target = keyframes[0] + 5

    with rr.server.Server(datasets={"video": rrd_dir}) as server:
        ds = server.client().get_dataset("video")
        source = DataSource(ds)
        dataset = RerunMapDataset(
            source,
            "frame",
            {
                "image": Field(
                    "/video:VideoStream:sample",
                    decode=VideoFrameDecoder(codec="h264"),
                ),
                "state": Field("/state:Scalars:scalars", decode=NumericDecoder()),
            },
        )
        with pytest.raises(ValueError, match="require an is_keyframe column"):
            dataset[target]

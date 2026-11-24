"""Collate YUV video, measure loader cost, and shuffle with fetch locality."""

from __future__ import annotations

import tempfile
import warnings
from pathlib import Path
from typing import Any

import av
import numpy as np
import torch
import torch.multiprocessing

import rerun as rr
from rerun.experimental.dataloader import (
    Field,
    NumericDecoder,
    VideoFrameDecoder,
)

NUM_FRAMES = 40
KEYFRAME_INTERVAL = 10


def write_episode(path: Path, recording_id: str) -> None:
    """Write an H.264 camera stream and joint states on `frame_index`."""
    container = av.open("/dev/null", "w", format="h264")
    stream = container.add_stream("libx264", rate=10)
    assert isinstance(stream, av.video.stream.VideoStream)
    stream.width = 64
    stream.height = 48
    stream.max_b_frames = 0
    stream.gop_size = KEYFRAME_INTERVAL
    # Signal BT.709 limited range so decoded frames carry color metadata.
    stream.codec_context.colorspace = 1
    stream.codec_context.color_primaries = 1
    stream.codec_context.color_trc = 1
    stream.codec_context.color_range = 1

    samples: list[bytes] = []
    keyframes: list[bool] = []
    frames = [
        av.VideoFrame.from_ndarray(
            np.full((48, 64, 3), 4 * i, dtype=np.uint8), format="rgb24"
        )
        for i in range(NUM_FRAMES)
    ]
    for frame in [*frames, None]:
        for packet in stream.encode(frame):
            samples.append(bytes(packet))
            keyframes.append(packet.is_keyframe)
    container.close()

    with rr.RecordingStream(
        "rerun_example_dataloader_performance", recording_id=recording_id
    ) as recording:
        recording.save(path)
        recording.log(
            "/camera", rr.VideoStream(codec=rr.VideoCodec.H264), static=True
        )
        frame_index = rr.TimeColumn("frame_index", sequence=range(NUM_FRAMES))
        recording.send_columns(
            "/camera",
            indexes=[frame_index],
            columns=rr.VideoStream.columns(
                sample=[np.frombuffer(s, dtype=np.uint8) for s in samples],
                is_keyframe=keyframes,
            ),
        )
        recording.send_columns(
            "/observation/joint_positions",
            indexes=[frame_index],
            columns=rr.Scalars.columns(
                scalars=np.random.default_rng(0).random((NUM_FRAMES, 7))
            ),
        )


def custom_state_collation(samples: list[dict[str, Any]]) -> torch.Tensor:
    return torch.stack([sample["state"] for sample in samples]).float()


def train_step(batch: dict[str, Any]) -> None:
    assert batch["state"].shape[-1] == 7


def main() -> None:
    # Rerun's tokio runtime is not fork-safe, so workers must use `spawn`.
    torch.multiprocessing.set_start_method("spawn", force=True)
    # `pin_memory=True` warns when no accelerator is available.
    warnings.filterwarnings(
        "ignore", message=".*pin_memory.*", category=UserWarning
    )

    device = "cuda" if torch.cuda.is_available() else "cpu"
    report_metrics = True

    with tempfile.TemporaryDirectory() as directory:
        paths = []
        for episode in ("episode_1", "episode_2"):
            path = Path(directory) / f"{episode}.rrd"
            write_episode(path, episode)
            paths.append(path)

        with rr.server.Server(datasets={"robot_episodes": paths}) as server:
            from rerun.experimental.dataloader import DataSource

            source = DataSource(server.client().get_dataset("robot_episodes"))
            state = Field(
                "/observation/joint_positions:Scalars:scalars",
                decode=NumericDecoder(),
            )
            yuv_fields = {
                "video": Field(
                    "/camera:VideoStream:sample",
                    decode=VideoFrameDecoder(
                        codec="h264", output_format="yuv420p"
                    ),
                ),
                "state": state,
            }
            fields = {
                "video": Field(
                    "/camera:VideoStream:sample",
                    decode=VideoFrameDecoder(codec="h264"),
                ),
                "state": state,
            }
            from rerun.experimental.dataloader import RerunIterableDataset

            dataset = RerunIterableDataset(
                source=source, index="frame_index", fields=yuv_fields
            )

            # region: yuv_collator
            from torch.utils.data import DataLoader

            from rerun.experimental.dataloader import Yuv420Collator

            loader = DataLoader(
                dataset, collate_fn=Yuv420Collator(), pin_memory=True
            )

            for batch in loader:
                batch["video"] = batch["video"].to_rgb(
                    device, non_blocking=True
                )
            # endregion: yuv_collator

            assert batch["video"].shape == (1, 3, 48, 64)

            # region: custom_collate
            from rerun.experimental.dataloader import Yuv420Frame

            def collate(samples: list[dict[str, Any]]) -> dict[str, Any]:
                return {
                    "video": Yuv420Frame.stack([
                        sample["video"] for sample in samples
                    ]),
                    "state": custom_state_collation(samples),
                }

            loader = DataLoader(dataset, collate_fn=collate, pin_memory=True)

            for batch in loader:
                batch["video"] = batch["video"].to_rgb(
                    device, non_blocking=True
                )
            # endregion: custom_collate

            # region: metrics
            from rerun.experimental.dataloader import RerunIterableDataset

            dataset = RerunIterableDataset(
                source=source,
                index="frame_index",
                fields=fields,
            )
            loader = DataLoader(
                dataset,
                batch_size=8,
                num_workers=4,
                multiprocessing_context="spawn",
            )

            for batch in loader:
                train_step(batch)

            if report_metrics:
                print(dataset.metrics.summary())
            # endregion: metrics

            from rerun.experimental.dataloader import BlockShuffle

            # region: block_shuffle
            ds = RerunIterableDataset(
                source=source,
                index="frame_index",
                fields=fields,
                shuffle_strategy=BlockShuffle(buffer_size=4096),
            )
            # endregion: block_shuffle

            assert sum(1 for _ in ds) == 2 * NUM_FRAMES


if __name__ == "__main__":
    main()

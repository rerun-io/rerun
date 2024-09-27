from __future__ import annotations

import os
from argparse import Namespace
from io import BytesIO
from uuid import uuid4

import rerun as rr
import rerun.blueprint as rrb

README = """\
# Video support
This test only works in the browser!

The video should show up both in 2D, 3D, and in the selection panel.

When moving the time cursor, a "loading" spinner should show briefly, while the video is seeking.
"""


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())

    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), timeless=True)

    # Log video asset which is referred to by frame references.
    video_path = os.path.dirname(__file__) + "/../../../tests/assets/video/Big_Buck_Bunny_1080_10s_av1.mp4"
    video_asset = rr.AssetVideo(path=video_path)
    rr.log("world/cam", video_asset, static=True)

    # Send automatically determined video frame timestamps.
    frame_timestamps_ns = video_asset.read_frame_timestamps_ns()
    rr.send_columns(
        "world/cam",
        # Note timeline values don't have to be the same as the video timestamps.
        times=[rr.TimeNanosColumn("video_time", frame_timestamps_ns)],
        components=[rr.VideoFrameReference.indicator(), rr.components.VideoTimestamp.nanoseconds(frame_timestamps_ns)],
    )

    rr.log(
        "world/cam",
        rr.Transform3D(
            translation=[10, 0, 0],
        ),
        static=True,  # Static, so it shows up in the "video_time" timeline!
    )

    rr.log(
        "world/cam",
        rr.Pinhole(
            resolution=[1920, 1080],
            focal_length=1920,
        ),
        static=True,  # Static, so it shows up in the "video_time" timeline!
    )


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)

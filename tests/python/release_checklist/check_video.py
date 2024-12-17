from __future__ import annotations

import os
from argparse import Namespace
from uuid import uuid4

import rerun as rr
import rerun.blueprint as rrb

README = """\
# Video support

Known issues:
* TODO(#8073): The last frame of the video is not working, so you might need to go back a couple of frames.

Make sure video plays on all views unless lack of support is mentioned:

* `AV1` is expected to work on web & release version native.
(In debug native this should inform that it is not supported in debug.)
* `AV1 frustum` should work if `AV1 video` works, showing a frustum with a video.
* `H.264` should work on web & native. On native this may complain about missing ffmpeg. If so, selecting it should inform you how to fix this.
* `H.265` should work on Chrome & Safari. Firefox does not support it.
* `VP9` works only on web, on native this should inform that the codec is not supported.

"""


def run(args: Namespace) -> None:
    rr.script_setup(args, f"{os.path.basename(__file__)}", recording_id=uuid4())

    rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), timeless=True)

    for codec in ["av1", "h264", "h265", "vp9"]:
        # Log video asset which is referred to by frame references.
        video_path = os.path.dirname(__file__) + f"/../../../tests/assets/video/Big_Buck_Bunny_1080_10s_{codec}.mp4"
        video_asset = rr.AssetVideo(path=video_path)
        rr.log(codec, video_asset, static=True)

        # Send automatically determined video frame timestamps.
        frame_timestamps_ns = video_asset.read_frame_timestamps_ns()
        rr.send_columns(
            codec,
            # Note timeline values don't have to be the same as the video timestamps.
            times=[rr.TimeNanosColumn("video_time", frame_timestamps_ns)],
            components=[
                rr.VideoFrameReference.indicator(),
                rr.components.VideoTimestamp.nanoseconds(frame_timestamps_ns),
            ],
        )

    # Use the av1 also in a 3D context
    rr.log(
        "av1",
        rr.Transform3D(
            translation=[10, 0, 0],
        ),
        static=True,  # Static, so it shows up in the "video_time" timeline!
    )
    rr.log(
        "av1",
        rr.Pinhole(
            resolution=[1920, 1080],
            focal_length=1920,
            camera_xyz=rr.ViewCoordinates.RBU,
        ),
        static=True,  # Static, so it shows up in the "video_time" timeline!
    )

    blueprint = rrb.Blueprint(
        rrb.Grid(
            rrb.TextDocumentView(origin="readme", name="Instructions"),
            rrb.Spatial3DView(origin="/", name="AV1 frustum", contents="av1"),
            rrb.Spatial2DView(origin="av1", name="AV1"),
            rrb.Spatial2DView(origin="h264", name="H.264"),
            rrb.Spatial2DView(origin="h265", name="H.265"),
            rrb.Spatial2DView(origin="vp9", name="VP9"),
        ),
    )

    rr.send_blueprint(blueprint, make_active=True, make_default=True)


if __name__ == "__main__":
    import argparse

    parser = argparse.ArgumentParser(description="Interactive release checklist")
    rr.script_add_args(parser)
    args = parser.parse_args()
    run(args)

"""Manual use of individual video frame references."""

import sys

import rerun as rr
import rerun.blueprint as rrb

if len(sys.argv) < 2:
    # TODO(#7354): Only mp4 is supported for now.
    print(f"Usage: {sys.argv[0]} <path_to_video.[mp4]>")
    sys.exit(1)

rr.init("rerun_example_asset_video_manual_frames", spawn=True)

# Log video asset which is referred to by frame references.
rr.log("video_asset", rr.AssetVideo(path=sys.argv[1]), static=True)

# Create two entities, showing the same video frozen at different times.
rr.log(
    "frame_1s",
    rr.VideoFrameReference(seconds=1.0, video_reference="video_asset"),
)
rr.log(
    "frame_2s",
    rr.VideoFrameReference(seconds=2.0, video_reference="video_asset"),
)

# Send blueprint that shows two 2D views next to each other.
rr.send_blueprint(rrb.Horizontal(rrb.Spatial2DView(origin="frame_1s"), rrb.Spatial2DView(origin="frame_2s")))

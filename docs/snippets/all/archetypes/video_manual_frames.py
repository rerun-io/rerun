"""Manual use of individual video frame references."""
# TODO(#7298): ⚠️ Video is currently only supported in the Rerun web viewer.
# TODO(#7420): This sample doesn't render yet.

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
    "frame_at_start",
    rr.VideoFrameReference(
        timestamp=rr.components.VideoTimestamp(seconds=0.0),
        video_reference="video_asset",
    ),
)
rr.log(
    "frame_at_one_second",
    rr.VideoFrameReference(
        timestamp=rr.components.VideoTimestamp(seconds=1.0),
        video_reference="video_asset",
    ),
)

# Send blueprint that shows two 2D views next to each other.
rr.send_blueprint(
    rrb.Horizontal(rrb.Spatial2DView(origin="frame_at_start"), rrb.Spatial2DView(origin="frame_at_one_second"))
)

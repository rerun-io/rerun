"""Log a video asset using manually created frame references."""
# TODO(#7298): ⚠️ Video is currently only supported in the Rerun web viewer.

import sys

import rerun as rr
import numpy as np

if len(sys.argv) < 2:
    # TODO(#7354): Only mp4 is supported for now.
    print(f"Usage: {sys.argv[0]} <path_to_video.[mp4]>")
    sys.exit(1)

rr.init("rerun_example_asset_video_manual_frames", spawn=True)

# Log video asset which is referred to by frame references.
rr.set_time_seconds("video_time", 0)  # Make sure it's available on the timeline used for the frame references.
rr.log("video", rr.AssetVideo(path=sys.argv[1]))

# Send frame references for every 0.1 seconds over a total of 10 seconds.
# Naturally, this will result in a choppy playback and only makes sense if the video is 10 seconds or longer.
# TODO(#7368): Point to example using `send_video_frames`.
#
# Use `send_columns` to send all frame references in a single call.
times = np.arange(0.0, 10.0, 0.1)
rr.send_columns(
    "video",
    times=[rr.TimeSecondsColumn("video_time", times)],
    components=[rr.VideoFrameReference.indicator(), rr.components.VideoTimestamp.seconds(times)],
)

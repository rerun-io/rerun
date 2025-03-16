"""Log a video asset using automatically determined frame references."""

import sys

import rerun as rr

if len(sys.argv) < 2:
    # TODO(#7354): Only mp4 is supported for now.
    print(f"Usage: {sys.argv[0]} <path_to_video.[mp4]>")
    sys.exit(1)

rr.init("rerun_example_asset_video_auto_frames", spawn=True)

# Log video asset which is referred to by frame references.
video_asset = rr.AssetVideo(path=sys.argv[1])
rr.log("video", video_asset, static=True)

# Send automatically determined video frame timestamps.
frame_timestamps_ns = video_asset.read_frame_timestamps_ns()
rr.send_columns(
    "video",
    # Note timeline values don't have to be the same as the video timestamps.
    indexes=[rr.TimeColumn("video_time", timedelta=1e-9 * frame_timestamps_ns)],
    columns=rr.VideoFrameReference.columns_nanoseconds(frame_timestamps_ns),
)

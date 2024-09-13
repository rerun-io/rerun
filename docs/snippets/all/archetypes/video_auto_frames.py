"""Log a video asset using automatically determined frame references."""
# TODO(#7298): ⚠️ Video is currently only supported in the Rerun web viewer.

import sys

import rerun as rr

if len(sys.argv) < 2:
    # TODO(#7354): Only mp4 is supported for now.
    print(f"Usage: {sys.argv[0]} <path_to_video.[mp4]>")
    sys.exit(1)

rr.init("rerun_example_asset_video_auto_frames", spawn=True)

# Log video asset which is referred to by frame references.
rr.set_time_seconds("video_time", 0)  # Make sure it's available on the timeline used for the frame references.
video_asset = rr.AssetVideo(path=sys.argv[1])
rr.log("video", video_asset)

# Send automatically determined video frame timestamps.
video_timestamps_ns = video_asset.read_frame_timestamps_ns()
rr.send_columns(
    "video",
    # Note timeline values don't have to be the same as the video timestamps.
    times=[rr.TimeNanosColumn("video_time", video_timestamps_ns)],
    components=[rr.VideoFrameReference.indicator(), rr.components.VideoTimestamp.nanoseconds(video_timestamps_ns)],
)

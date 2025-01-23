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
video_asset = rr.AssetVideo(path=sys.argv[1])
rr.log("video", video_asset, static=True)

# Send automatically determined video frame timestamps.
frame_timestamps_ns = video_asset.read_frame_timestamps_ns()
rr.send_columns_v2(
    "video",
    # Note timeline values don't have to be the same as the video timestamps.
    indexes=[rr.TimeNanosColumn("video_time", frame_timestamps_ns)],
    # TODO: some archetype extension shenanigans
    columns=rr.VideoFrameReference.columns(timestamp=frame_timestamps_ns),
)

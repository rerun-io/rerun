"""Log a video file."""

import sys

import rerun as rr

if len(sys.argv) < 2:
    print(f"Usage: {sys.argv[0]} <path_to_video.[mp4]>")
    sys.exit(1)

rr.init("rerun_example_asset_video", spawn=True)

rr.log(f"world/video", rr.AssetVideo(path=sys.argv[1]))

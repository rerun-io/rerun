"""
Log a video file.

Note: This example is currently web-only.
"""

import sys
import time

import rerun as rr

if len(sys.argv) < 2:
    print(f"Usage: {sys.argv[0]} <path_to_asset.[mp4]>")
    sys.exit(1)

rr.init("rerun_example_asset_video")
rr.serve()

# At the moment, the entire video is logged as one component, meaning it gets a single time point.
# That means the video can't be played back at all, because the timeline can never move from
# that single time point.
# TODO(jan): ensure that videos can always be played back in isolation
rr.set_time_seconds("video_time", 0)

video_paths = sys.argv[1:]
for i in range(len(video_paths)):
    rr.log(f"world/asset/{i}", rr.AssetVideo(path=video_paths[i]))

for t in range(120):
    rr.set_time_seconds("video_time", t)
    rr.log("world/end", rr.Points3D([]))

# wait for ctrl-c
while True:
    try:
        time.sleep(1)
    except KeyboardInterrupt:
        break

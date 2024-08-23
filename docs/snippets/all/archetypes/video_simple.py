"""Log a simple 3D asset."""

import sys
import time

import rerun as rr

if len(sys.argv) < 2:
    print(f"Usage: {sys.argv[0]} <path_to_asset.[mp4]>")
    sys.exit(1)

rr.init("rerun_example_asset_video")
rr.serve()

rr.set_time_seconds("stuff", 0)
rr.log("world", rr.ViewCoordinates.RIGHT_HAND_Z_UP, static=True)  # Set an up-axis

video_paths = sys.argv[1:]
for i in range(len(video_paths)):
    rr.log(f"world/asset/{i}", rr.AssetVideo(path=video_paths[i]))

for t in range(60):
    rr.set_time_seconds("stuff", t)
    rr.log("world/end", rr.Points3D([]))

# wait for ctrl-c
while True:
    try:
        time.sleep(1)
    except KeyboardInterrupt:
        break

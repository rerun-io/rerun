"""Log a simple 3D asset."""
import sys

import rerun as rr

if len(sys.argv) < 2:
    print(f"Usage: {sys.argv[0]} <path_to_asset.[gltf|glb|obj|stl]>")
    sys.exit(1)

rr.init("rerun_example_asset3d", spawn=True)

rr.log("world", rr.ViewCoordinates.RIGHT_HAND_Z_UP, static=True)  # Set an up-axis
rr.log("world/asset", rr.Asset3D(path=sys.argv[1]))

"""Log a a simple 3D asset."""
import sys

import rerun as rr
import rerun.experimental as rr2

if len(sys.argv) < 2:
    print(f"Usage: {sys.argv[0]} <path_to_asset.[gltf|glb]>")
    sys.exit(1)

rr.init("rerun_example_asset3d_simple", spawn=True)

# TODO(#2816): some viewcoords would be nice here
rr2.log("asset", rr2.Asset3D.from_file(sys.argv[1]))

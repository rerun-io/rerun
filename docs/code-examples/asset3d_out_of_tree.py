"""Log a simple 3D asset with an out-of-tree transform which will not affect its children."""
import sys

import numpy as np
import rerun as rr
import rerun.experimental as rr2

if len(sys.argv) < 2:
    print(f"Usage: {sys.argv[0]} <path_to_asset.[gltf|glb]>")
    sys.exit(1)

rr.init("rerun_example_asset3d_out_of_tree", spawn=True)

rr2.log("world", rr2.ViewCoordinates.RIGHT_HAND_Z_UP, timeless=True)  # Set an up-axis

rr.set_time_sequence("frame", 0)
rr2.log("world/asset", rr2.Asset3D.from_file(sys.argv[1]))
# Those points will not be affected by their parent's out-of-tree transform!
rr2.log(
    "world/asset/points",
    rr2.Points3D(np.vstack([xyz.ravel() for xyz in np.mgrid[3 * [slice(-10, 10, 10j)]]]).T),
)

asset = rr2.Asset3D.from_file(sys.argv[1])
for i in range(1, 20):
    rr.set_time_sequence("frame", i)

    translation = rr2.dt.TranslationRotationScale3D(translation=[0, 0, i - 10.0])
    rr2.log_components("asset", [rr2.cmp.OutOfTreeTransform3DBatch(translation)])

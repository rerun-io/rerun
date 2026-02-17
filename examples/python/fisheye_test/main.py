"""Log both a Pinhole and a Fisheye camera for comparison."""

import numpy as np
import rerun as rr

rr.init("fisheye_test", spawn=True)
rng = np.random.default_rng(12345)

# Rotate cameras to face horizontally along +X, with Z-up world.
# Camera convention is RDF: cam-X=Right, cam-Y=Down, cam-Z=Forward.
# Rotation matrix columns = where each camera axis maps in world coords:
#   col 0 (cam-X Right  -> world -Y):  (0, -1, 0)
#   col 1 (cam-Y Down   -> world -Z):  (0, 0, -1)
#   col 2 (cam-Z Forward -> world +X): (1, 0, 0)
camera_rotation = np.array([
    [0,  0, 1],
    [-1, 0, 0],
    [0, -1, 0],
], dtype=np.float32)

# Place cameras at different positions, rotated to face horizontally.
rr.log(
    "world/pinhole_parent",
    rr.Transform3D(translation=[0, 0, 0], mat3x3=camera_rotation),
)
rr.log(
    "world/fisheye_parent",
    rr.Transform3D(translation=[0, 2, 0], mat3x3=camera_rotation),
)

# -- Pinhole camera --
rr.log(
    "world/pinhole_parent/cam",
    rr.Pinhole(
        focal_length=200.0,
        width=640,
        height=480,
        image_plane_distance=0.4,
        color=[0, 128, 255],
        line_width=0.005,
    ),
)
image_pinhole = rng.uniform(0, 255, size=[480, 640, 3]).astype(np.uint8)
rr.log("world/pinhole_parent/cam", rr.Image(image_pinhole))

# -- Fisheye camera --
rr.log(
    "world/fisheye_parent/cam",
    rr.Fisheye(
        focal_length=200.0,
        width=640,
        height=480,
        distortion_coefficients=[0.1, -0.01, 0.001, 0.0],
        image_plane_distance=0.4,
        color=[255, 128, 0],
        line_width=0.005,
    ),
)
image_fisheye = rng.uniform(0, 255, size=[480, 640, 3]).astype(np.uint8)
rr.log("world/fisheye_parent/cam", rr.Image(image_fisheye))

# Some reference geometry in front of the cameras (along +X).
rr.log(
    "world/points",
    rr.Points3D(
        [(3.0, 0.0, 0.0), (3.0, 1.0, 0.0), (3.0, 2.0, 0.0), (3.0, 1.0, 1.0)],
        radii=0.05,
    ),
)

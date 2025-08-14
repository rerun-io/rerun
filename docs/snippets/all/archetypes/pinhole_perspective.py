"""Logs a point cloud and a perspective camera looking at it."""

import rerun as rr

rr.init("rerun_example_pinhole_perspective", spawn=True)

rr.log(
    "world/cam",
    rr.Pinhole(
        fov_y=0.7853982,
        aspect_ratio=1.7777778,
        camera_xyz=rr.ViewCoordinates.RUB,
        image_plane_distance=0.1,
        color=[255, 128, 0],
        line_width=0.003,
    ),
)

rr.log("world/points", rr.Points3D([(0.0, 0.0, -0.5), (0.1, 0.1, -0.5), (-0.1, -0.1, -0.5)], radii=0.025))

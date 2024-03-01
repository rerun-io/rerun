"""Logs a point cloud and a perspective camera looking at it."""
import rerun as rr

rr.init("rerun_example_pinhole_perspective", spawn=True)

rr.log("world/cam", rr.Pinhole(fov_y=0.7853982, aspect_ratio=1.7777778, camera_xyz=rr.ViewCoordinates.RUB))

rr.log("world/points", rr.Points3D([(0.0, 0.0, -0.5), (0.1, 0.1, -0.5), (-0.1, -0.1, -0.5)]))

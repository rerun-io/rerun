"""Shows how to manually associate one or more indicator components with arbitrary data."""
import rerun as rr

rr.init("rerun_example_manual_indicator", spawn=True)

# Specify both a Mesh3D and a Points3D indicator component so that the data is shown as both a
# 3D mesh _and_ a point cloud by default.
rr.log(
    "points_and_mesh",
    [
        rr.Points3D.indicator(),
        rr.Mesh3D.indicator(),
        rr.components.Position3DBatch([[0.0, 0.0, 0.0], [10.0, 0.0, 0.0], [0.0, 10.0, 0.0]]),
        rr.components.ColorBatch([[1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]]),
        rr.components.RadiusBatch([1.0]),
    ],
)

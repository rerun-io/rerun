"""Demonstrates usage of the new partial updates APIs."""

import rerun as rr

rr.init("rerun_example_points3d_partial_updates", spawn=True)

positions = [[i, 0, 0] for i in range(0, 10)]

rr.set_time_sequence("frame", 0)
rr.log("points", rr.Points3D(positions))

for i in range(0, 10):
    colors = [[20, 200, 20] if n < i else [200, 20, 20] for n in range(0, 10)]
    radii = [0.6 if n < i else 0.2 for n in range(0, 10)]

    rr.set_time_sequence("frame", i)
    rr.log("points", [rr.Points3D.indicator(), rr.components.ColorBatch(colors), rr.components.RadiusBatch(radii)])
    # TODO(cmc): implement new APIs and use them!
    # rr.log("points", rr.Points3D.update_fields(radii=radii, colors=colors))

rr.set_time_sequence("frame", 20)
rr.log(
    "points",
    [
        rr.Points3D.indicator(),
        rr.components.Position3DBatch(positions),
        rr.components.RadiusBatch(0.3),
        rr.components.ColorBatch([]),
        rr.components.TextBatch([]),
        rr.components.ShowLabelsBatch([]),
        rr.components.ClassIdBatch([]),
        rr.components.KeypointIdBatch([]),
    ],
)
# TODO(cmc): implement new APIs and use them!
# rr.log("points", rr.Points3D.clear_fields().update_fields(positions=positions, radii=0.3))

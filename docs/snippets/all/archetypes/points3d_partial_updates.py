"""Update specific properties of a point cloud over time."""

import rerun as rr

rr.init("rerun_example_points3d_partial_updates", spawn=True)

positions = [[i, 0, 0] for i in range(10)]

rr.set_time("frame", sequence=0)
rr.log("points", rr.Points3D(positions))

for i in range(10):
    colors = [[20, 200, 20] if n < i else [200, 20, 20] for n in range(10)]
    radii = [0.6 if n < i else 0.2 for n in range(10)]

    # Update only the colors and radii, leaving everything else as-is.
    rr.set_time("frame", sequence=i)
    rr.log("points", rr.Points3D.from_fields(radii=radii, colors=colors))

# Update the positions and radii, and clear everything else in the process.
rr.set_time("frame", sequence=20)
rr.log("points", rr.Points3D.from_fields(clear_unset=True, positions=positions, radii=0.3))

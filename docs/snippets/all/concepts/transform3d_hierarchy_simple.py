"""Logs a simple transform hierarchy."""

import rerun as rr

rr.init("rerun_example_transform3d_hierarchy_simple", spawn=True)

# Log entities at their hierarchy positions
rr.log("sun", rr.Ellipsoids3D(centers=[0, 0, 0], half_sizes=[1, 1, 1], colors=[255, 200, 10]))
rr.log("sun/planet", rr.Ellipsoids3D(centers=[0, 0, 0], half_sizes=[0.4, 0.4, 0.4], colors=[40, 80, 200]))
rr.log("sun/planet/moon", rr.Ellipsoids3D(centers=[0, 0, 0], half_sizes=[0.15, 0.15, 0.15], colors=[180, 180, 180]))

# Define transforms - each describes the relationship to its parent
rr.log("sun/planet", rr.Transform3D(translation=[6.0, 0.0, 0.0]))  # Planet 6 units from sun
rr.log("sun/planet/moon", rr.Transform3D(translation=[3.0, 0.0, 0.0]))  # Moon 3 units from planet

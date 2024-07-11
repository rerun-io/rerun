"""Logs a transforms transform hierarchy."""

import numpy as np
import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_transform3d_hierarchy", spawn=True)

# One space with the sun in the center, and another one with the planet.
rr.send_blueprint(
    rrb.Horizontal(rrb.Spatial3DView(origin="sun"), rrb.Spatial3DView(origin="sun/planet", contents="sun/**"))
)

rr.set_time_seconds("sim_time", 0)

# Planetary motion is typically in the XY plane.
rr.log("/", rr.ViewCoordinates.RIGHT_HAND_Z_UP, static=True)

# Setup points, all are in the center of their own space:
# TODO(#1361): Should use spheres instead of points.
rr.log("sun", rr.Points3D([0.0, 0.0, 0.0], radii=1.0, colors=[255, 200, 10]))
rr.log("sun/planet", rr.Points3D([0.0, 0.0, 0.0], radii=0.4, colors=[40, 80, 200]))
rr.log("sun/planet/moon", rr.Points3D([0.0, 0.0, 0.0], radii=0.15, colors=[180, 180, 180]))

# Draw fixed paths where the planet & moon move.
d_planet = 6.0
d_moon = 3.0
angles = np.arange(0.0, 1.01, 0.01) * np.pi * 2
circle = np.array([np.sin(angles), np.cos(angles), angles * 0.0]).transpose()
rr.log("sun/planet_path", rr.LineStrips3D(circle * d_planet))
rr.log("sun/planet/moon_path", rr.LineStrips3D(circle * d_moon))

# Movement via transforms.
for i in range(0, 6 * 120):
    time = i / 120.0
    rr.set_time_seconds("sim_time", time)
    r_moon = time * 5.0
    r_planet = time * 2.0

    rr.log(
        "sun/planet",
        rr.Transform3D(
            translation=[np.sin(r_planet) * d_planet, np.cos(r_planet) * d_planet, 0.0],
            rotation=rr.RotationAxisAngle(axis=(1, 0, 0), degrees=20),
        ),
    )
    rr.log(
        "sun/planet/moon",
        rr.Transform3D(
            translation=[np.cos(r_moon) * d_moon, np.sin(r_moon) * d_moon, 0.0],
            from_parent=True,
        ),
    )

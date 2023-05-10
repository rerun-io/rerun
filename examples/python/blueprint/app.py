# Fork transform3d

import rerun as rr
import numpy as np
import math
from scipy.spatial.transform import Rotation

rr.init("Space", spawn=True)

rr.set_time_seconds("sim_time", 0)
sun_to_planet_distance = 6.0
planet_to_moon_distance = 3.0
rotation_speed_planet = 2.0
rotation_speed_moon = 5.0

# Planetary motion is typically in the XY plane.
rr.log_view_coordinates("transforms3d", up="+Z", timeless=True)
rr.log_view_coordinates("transforms3d/sun", up="+Z", timeless=True)
rr.log_view_coordinates("transforms3d/sun/planet", up="+Z", timeless=True)
rr.log_view_coordinates("transforms3d/sun/planet/moon", up="+Z", timeless=True)

# All are in the center of their own space:
rr.log_point("transforms3d/sun", [0.0, 0.0, 0.0], radius=1.0, color=[255, 200, 10])
rr.log_point("transforms3d/sun/planet", [0.0, 0.0, 0.0], radius=0.4, color=[40, 80, 200])
rr.log_point("transforms3d/sun/planet/moon", [0.0, 0.0, 0.0], radius=0.15, color=[180, 180, 180])

# "dust" around the "planet" (and inside, don't care)
# distribution is quadratically higher in the middle
radii = np.random.rand(200) * planet_to_moon_distance * 0.5
angles = np.random.rand(200) * math.tau
height = np.power(np.random.rand(200), 0.2) * 0.5 - 0.5
rr.log_points(
    "transforms3d/sun/planet/dust",
    np.array([np.sin(angles) * radii, np.cos(angles) * radii, height]).transpose(),
    colors=[80, 80, 80],
    radii=0.025,
)

# paths where the planet & moon move
angles = np.arange(0.0, 1.01, 0.01) * math.tau
circle = np.array([np.sin(angles), np.cos(angles), angles * 0.0]).transpose()
rr.log_line_strip(
    "transforms3d/sun/planet_path",
    circle * sun_to_planet_distance,
)
rr.log_line_strip(
    "transforms3d/sun/planet/moon_path",
    circle * planet_to_moon_distance,
)

# movement via transforms
for i in range(0, 6 * 120):
    time = i / 120.0
    rr.set_time_seconds("sim_time", time)
    rotation_q = [0, 0, 0, 1]

    rr.log_rigid3(
        "transforms3d/sun/planet",
        parent_from_child=(
            [
                math.sin(time * rotation_speed_planet) * sun_to_planet_distance,
                math.cos(time * rotation_speed_planet) * sun_to_planet_distance,
                0.0,
            ],
            Rotation.from_euler("x", 20, degrees=True).as_quat(),
        ),
    )
    rr.log_rigid3(
        "transforms3d/sun/planet/moon",
        child_from_parent=(
            [
                math.cos(time * rotation_speed_moon) * planet_to_moon_distance,
                math.sin(time * rotation_speed_moon) * planet_to_moon_distance,
                0.0,
            ],
            rotation_q,
        ),
    )

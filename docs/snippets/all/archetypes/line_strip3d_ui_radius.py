"""Log lines with ui points & scene unit radii."""

import rerun as rr

rr.init("rerun_example_line_strip3d_ui_radius", spawn=True)

# A blue line with a scene unit radii of 0.01.
points = [[0, 0, 0], [0, 0, 1], [1, 0, 0], [1, 0, 1]]
rr.log(
    "scene_unit_line",
    rr.LineStrips3D(
        [points],
        # By default, radii are interpreted as world-space units.
        radii=0.01,
        colors=[0, 0, 255],
    ),
)

# A red line with a ui point radii of 5.
# UI points are independent of zooming in Views, but are sensitive to the application UI scaling.
# For 100% ui scaling, UI points are equal to pixels.
points = [[3, 0, 0], [3, 0, 1], [4, 0, 0], [4, 0, 1]]
rr.log(
    "ui_points_line",
    rr.LineStrips3D(
        [points],
        # rr.Radius.ui_points produces radii that the viewer interprets as given in ui points.
        radii=rr.Radius.ui_points(5.0),
        colors=[255, 0, 0],
    ),
)

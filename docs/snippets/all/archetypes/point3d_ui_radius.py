"""Log some points with ui points & scene unit radii."""

import rerun as rr

rr.init("rerun_example_points3d_ui_radius", spawn=True)

# Two blue points with scene unit radii of 0.1 and 0.3.
rr.log(
    "scene_unit_points",
    rr.Points3D(
        [[0, 1, 0], [1, 1, 1]],
        # By default, radii are interpreted as world-space units.
        radii=[0.1, 0.3],
        colors=[0, 0, 255],
    ),
)

# Two red points with ui point radii of 40 and 60.
# Ui points are independent of zooming in Views, but are sensitive to the application ui scaling.
# For 100% ui scaling, ui points are equal to pixels.
rr.log(
    "ui_points_points",
    rr.Points3D(
        [[0, 0, 0], [1, 0, 1]],
        # rr.Radius.ui_points produces radii that the viewer interprets as given in ui units.
        radii=rr.Radius.ui_points([40.0, 60.0]),
        colors=[255, 0, 0],
    ),
)

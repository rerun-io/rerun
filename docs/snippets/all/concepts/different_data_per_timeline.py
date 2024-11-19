"""Log different data on different timelines."""

import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_different_data_per_timeline", spawn=True)

rr.set_time_sequence("blue timeline", 0)
rr.set_time_seconds("red timeline", 0.0)
rr.log("points", rr.Points2D([[0, 0], [1, 1]], radii=rr.Radius.ui_points(10.0)))

# Log a red color on one timeline.
rr.reset_time()  # Clears all set timeline info.
rr.set_time_seconds("red timeline", 1.0)
rr.log("points", [rr.components.Color(0xFF0000FF)])

# And a blue color on the other.
rr.reset_time()  # Clears all set timeline info.
rr.set_time_sequence("blue timeline", 1)
rr.log("points", [rr.components.Color(0x0000FFFF)])


# Set view bounds:
rr.send_blueprint(rrb.Spatial2DView(visual_bounds=rrb.VisualBounds2D(x_range=[-1, 2], y_range=[-1, 2])))

import numpy as np
import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_temporal_segments", spawn=True)

rr.set_time_sequence("t", 0)
rr.log(
    "segments",
    rr.LineStrips2D(np.array([[[0, 0], [1, 1]]])),
)

rr.set_time_sequence("t", 1)
rr.log(
    "segments",
    rr.LineStrips2D(np.array([[[1, 1], [2, 0]]])),
)

rr.set_time_sequence("t", 2)
rr.log(
    "segments",
    rr.LineStrips2D(np.array([[[2, 0], [3, 1]]])),
)

rr.send_blueprint(rrb.Blueprint(
    rrb.Spatial2DView(
        visual_bounds=rrb.VisualBounds2D(x_range=[-1, 4], y_range=[-2, 2]),
        time_ranges=rrb.VisibleTimeRange(
            "t",
            start=rrb.TimeRangeBoundary.infinite(),
            end=rrb.TimeRangeBoundary.infinite(),
        ),
    ),
    rrb.BlueprintPanel(state="collapsed"),
    rrb.TimePanel(state="collapsed"),
))

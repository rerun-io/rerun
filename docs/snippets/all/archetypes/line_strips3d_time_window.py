"""Log line strips over time and view a sliding window (e.g. trajectories)."""

import math

import rerun as rr
import rerun.blueprint as rrb


def point(t: float, phase: float) -> list[float]:
    # Sample a point on a helix.
    angle = 0.5 * t + phase
    return [math.cos(angle), math.sin(angle), 0.1 * t]


rr.init("rerun_example_line_strips3d_time_window", spawn=True)

# Configure the visible time range in the blueprint.
# You can also override this per entity.
rr.send_blueprint(
    rrb.Spatial3DView(
        origin="/",
        time_ranges=rrb.VisibleTimeRange(
            "time",
            start=rrb.TimeRangeBoundary.cursor_relative(seconds=-5.0),
            end=rrb.TimeRangeBoundary.cursor_relative(),
        ),
    )
)

# Log the line strip increments with timestamps.
for i in range(600):
    t0 = i / 30.0
    t1 = (i + 1) / 30.0

    rr.set_time("time", duration=t1)
    rr.log(
        "trails",
        rr.LineStrips3D(
            [
                [point(t0, 0.0), point(t1, 0.0)],
                [point(t0, math.pi), point(t1, math.pi)],
            ],
            colors=[[255, 120, 0], [0, 180, 255]],
            radii=0.02,
        ),
    )

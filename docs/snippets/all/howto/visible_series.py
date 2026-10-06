"""Show one series of a multi-value `Scalars` batch."""

from __future__ import annotations

import math

import rerun as rr
import rerun.blueprint as rrb

rr.init("rerun_example_visible_series", spawn=True)

for t in range(50):
    rr.set_time("step", sequence=t)
    positions = [math.sin(t / 10 + j) for j in range(7)]
    rr.log("robot/joint_positions", rr.Scalars(positions))

# Show only the third joint
rr.send_blueprint(
    rrb.TimeSeriesView(
        overrides={
            "robot/joint_positions": rr.SeriesLines(
                visible_series=[False, False, True, False, False, False, False]
            ),
        },
    ),
)

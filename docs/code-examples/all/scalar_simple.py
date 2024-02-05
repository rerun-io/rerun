"""Log a scalar over time."""
import math

import rerun as rr

rr.init("rerun_example_scalar", spawn=True)

# Set up plot styling: Logged timeless since it never changes and affects all timelines.
rr.log("scalar", rr.SeriesPoint(), timeless=True)

# Log the data on a timeline called "step".
for step in range(0, 64):
    rr.set_time_sequence("step", step)
    rr.log("scalar", rr.Scalar(math.sin(step / 10.0)))

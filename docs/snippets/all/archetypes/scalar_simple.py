"""Log a scalar over time."""

import math

import rerun as rr

rr.init("rerun_example_scalar", spawn=True)

# Log the data on a timeline called "step".
for step in range(64):
    rr.set_time("step", sequence=step)
    rr.log("scalar", rr.Scalar(math.sin(step / 10.0)))

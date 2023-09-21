"""Log a scalar over time."""
import math

import rerun as rr
import rerun.experimental as rr2

rr.init("rerun_example_scalar", spawn=True)

for step in range(0, 64):
    rr.set_time_sequence("step", step)
    rr2.log("scalar", rr2.TimeSeriesScalar(math.sin(step / 10.0)))

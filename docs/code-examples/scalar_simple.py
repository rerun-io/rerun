"""Log a scalar over time."""
import math

import rerun as rr

rr.init("rerun_example_scalar")
rr.save("/tmp/scalars.rrd")

for step in range(0, 64000):
    rr.set_time_sequence("step", step)
    rr.log("scalar", rr.TimeSeriesScalar(math.sin(step / 10.0)))

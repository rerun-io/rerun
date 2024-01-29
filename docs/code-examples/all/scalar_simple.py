"""Log a scalar over time."""
import math

import rerun as rr

rr.init("rerun_example_scalar")
rr.save("/tmp/scalars.py")

for step in range(0, 64):
    rr.set_time_sequence("step", step)
    rr.log("scalar", rr.TimeSeriesScalar(math.sin(step / 10.0)))
    rr.log("scalar", rr.Clear(recursive=True))

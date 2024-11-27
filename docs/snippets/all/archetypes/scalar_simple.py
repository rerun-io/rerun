"""Log a scalar over time."""

import math

import rerun as rr

rr.init("rerun_example_scalar", spawn=True)

rr.set_time_sequence("step", 0)
rr.log("scalar", rr.Scalar(1.0))

rr.set_time_sequence("step", 0)
rr.log("scalar", rr.Scalar(10.0))

rr.set_time_sequence("step", 5)
rr.log("scalar", rr.Scalar(10.0))

rr.set_time_sequence("step", 5)
rr.log("scalar", rr.Scalar(0.0))

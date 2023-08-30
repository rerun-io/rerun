"""Log a scalar over time."""
import numpy as np
import rerun as rr

rr.init("rerun_example_scalar", spawn=True)
rng = np.random.default_rng(12345)

value = 1.0
for step in range(100):
    rr.set_time_sequence("step", step)

    value += rng.normal()
    rr.log_scalar("scalar", value)

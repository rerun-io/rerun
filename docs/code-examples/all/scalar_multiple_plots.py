"""Log a scalar over time."""

from math import cos, sin, tau

import numpy as np
import rerun as rr

rr.init("rerun_example_scalar_multiple_plots", spawn=True)
lcg_state = np.int64(0)

# Set up plot styling:
# They are logged timeless as they don't change over time and apply to all timelines.
# Log two lines series under a shared root so that they show in the same plot by default.
rr.log("trig/sin", rr.SeriesLine(color=[255, 0, 0]), timeless=True)
rr.log("trig/cos", rr.SeriesLine(color=[0, 255, 0]), timeless=True)
# Log scattered points under a different root so that they shows in a different plot by default.
rr.log("scatter/lcg", rr.SeriesPoint(), timeless=True)

# Log the data on a timeline called "step".
for t in range(0, int(tau * 2 * 100.0)):
    rr.set_time_sequence("step", t)

    rr.log("trig/sin", rr.Scalar(sin(float(t) / 100.0), text="sin(0.01t)"))
    rr.log("trig/cos", rr.Scalar(cos(float(t) / 100.0), text="cos(0.01t)"))

    lcg_state = (1140671485 * lcg_state + 128201163) % 16777216  # simple linear congruency generator
    rr.log("scatter/lcg", rr.Scalar(lcg_state))

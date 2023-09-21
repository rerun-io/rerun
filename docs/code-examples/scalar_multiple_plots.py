"""Log a scalar over time."""

from math import cos, sin, tau

import numpy as np
import rerun as rr
import rerun.experimental as rr2

rr.init("rerun_example_scalar_multiple_plots", spawn=True)
lcg_state = np.int64(0)

for t in range(0, int(tau * 2 * 100.0)):
    rr.set_time_sequence("step", t)

    # Log two time series under a shared root so that they show in the same plot by default.
    rr2.log("trig/sin", rr2.TimeSeriesScalar(sin(float(t) / 100.0), label="sin(0.01t)", color=[255, 0, 0]))
    rr2.log("trig/cos", rr2.TimeSeriesScalar(cos(float(t) / 100.0), label="cos(0.01t)", color=[0, 255, 0]))

    # Log scattered points under a different root so that they shows in a different plot by default.
    lcg_state = (1140671485 * lcg_state + 128201163) % 16777216  # simple linear congruency generator
    rr2.log("scatter/lcg", rr2.TimeSeriesScalar(lcg_state, scattered=True))

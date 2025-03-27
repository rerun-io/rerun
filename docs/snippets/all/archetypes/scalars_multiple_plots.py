"""Log a scalar over time."""

from math import cos, sin, tau

import numpy as np
import rerun as rr

rr.init("rerun_example_scalar_multiple_plots", spawn=True)
lcg_state = np.int64(0)

# Set up plot styling:
# They are logged as static as they don't change over time and apply to all timelines.
# Log two lines series under a shared root so that they show in the same plot by default.
rr.log("trig/sin", rr.SeriesLines(colors=[255, 0, 0], names="sin(0.01t)"), static=True)
rr.log("trig/cos", rr.SeriesLines(colors=[0, 255, 0], names="cos(0.01t)"), static=True)
# Log scattered points under a different root so that they show in a different plot by default.
rr.log("scatter/lcg", rr.SeriesPoints(), static=True)

# Log the data on a timeline called "step".
for t in range(int(tau * 2 * 100.0)):
    rr.set_time("step", sequence=t)

    rr.log("trig/sin", rr.Scalars(sin(float(t) / 100.0)))
    rr.log("trig/cos", rr.Scalars(cos(float(t) / 100.0)))

    lcg_state = (1140671485 * lcg_state + 128201163) % 16777216  # simple linear congruency generator
    rr.log("scatter/lcg", rr.Scalars(lcg_state.astype(np.float64)))

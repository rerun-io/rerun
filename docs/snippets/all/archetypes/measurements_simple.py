"""Log scalar measurements with variances over time."""

import math

import rerun as rr

rr.init("rerun_example_measurements_simple", spawn=True)

# Two parallel pressure sensors (in Pa), each with slowly drifting variance.
for step in range(64):
    rr.set_time("step", sequence=step)
    pressures = [
        101_325.0 + 50.0 * math.sin(step / 10.0),
        101_300.0 + 30.0 * math.cos(step / 8.0),
    ]
    variances = [
        100.0 + 25.0 * math.sin(step / 7.0),
        80.0 + 15.0 * math.cos(step / 11.0),
    ]
    rr.log(
        "pressure",
        rr.Measurements(values=pressures, variances=variances, units="Pa"),
    )

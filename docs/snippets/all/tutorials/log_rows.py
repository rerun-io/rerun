#!/usr/bin/env python3
"""Use the log APIs to log scalars over time."""

from __future__ import annotations

import math

import rerun as rr

rr.init("rerun_example_log_rows")
rr.stdout()

NUM_STEPS = 100_000
COEFF = 10.0 / NUM_STEPS

for step in range(0, NUM_STEPS):
    # Set the `step` timeline in the logging context to the current time.
    rr.set_time_sequence("step", step)

    # Log a new row containing a single scalar.
    # This will inherit from the logging context, and thus be logged at the current `step`.
    rr.log("scalar", rr.Scalar(math.sin(step * COEFF)))

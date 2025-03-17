"""
Update a scalar over time.

See also the `scalar_column_updates` example, which achieves the same thing in a single operation.
"""

from __future__ import annotations

import math

import rerun as rr

rr.init("rerun_example_scalar_row_updates", spawn=True)

for step in range(64):
    rr.set_time("step", sequence=step)
    rr.log("scalars", rr.Scalar(math.sin(step / 10.0)))

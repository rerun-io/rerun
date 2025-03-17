"""
Update custom user-defined values over time.

See also the `any_values_column_updates` example, which achieves the same thing in a single operation.
"""

from __future__ import annotations

import math

import rerun as rr

rr.init("rerun_example_any_values_row_updates", spawn=True)

for step in range(64):
    rr.set_time("step", sequence=step)
    rr.log("/", rr.AnyValues(sin=math.sin(step / 10.0), cos=math.cos(step / 10.0)))

"""
Update a scalar over time, in a single operation.

This is semantically equivalent to the `scalar_row_updates` example, albeit much faster.
"""

from __future__ import annotations

import numpy as np
import rerun as rr

rr.init("rerun_example_scalar_column_updates", spawn=True)

times = np.arange(0, 64)
scalars = np.sin(times / 10.0)

rr.send_columns(
    "scalars",
    indexes=[rr.TimeColumn("step", sequence=times)],
    columns=rr.Scalar.columns(scalar=scalars),
)

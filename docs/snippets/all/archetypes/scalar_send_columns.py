"""Use the `send_columns` API to send scalars over time in a single call."""

from __future__ import annotations

import numpy as np
import rerun as rr

rr.init("rerun_example_scalar_send_columns", spawn=True)

times = np.arange(0, 64)
scalars = np.sin(times / 10.0)

rr.send_columns(
    "scalars",
    times=[rr.TimeSequenceColumn("step", times)],
    components=[rr.components.ScalarBatch(scalars)],
)

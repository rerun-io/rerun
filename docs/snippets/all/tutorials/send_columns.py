#!/usr/bin/env python3
"""Very minimal test of using the send column APIs."""

from __future__ import annotations

import numpy as np
import rerun as rr

rr.init("rerun_example_send_columns", spawn=True)

times = np.arange(0, 64)
scalars = np.sin(times / 10.0)

rr.send_columns(
    "scalars",
    times=[rr.TimeSequenceBatch("step", times)],
    components=[rr.components.ScalarBatch(scalars)],
)

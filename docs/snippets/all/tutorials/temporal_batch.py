#!/usr/bin/env python3
"""Very minimal test of using the temporal batch APIs.."""

from __future__ import annotations

import numpy as np
import rerun as rr

rr.init("rerun_example_temporal_batch", spawn=True)

times = np.arange(0, 64)
scalars = np.sin(times / 10.0)

rr.log_temporal_batch(
    "scalars",
    times=[rr.TimeSequenceBatch("step", times)],
    components=[rr.components.ScalarBatch(scalars)],
)

#!/usr/bin/env python3
"""Log a scalar scalar batch."""

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


rng = np.random.default_rng(12345)

times = np.arange(0, 10)
positions = rng.uniform(-5, 5, size=[100, 3])
colors = rng.uniform(0, 255, size=[100, 3])
radii = rng.uniform(0, 1, size=[100])

rr.log_temporal_batch(
    "points",
    times=[rr.TimeSequenceBatch("step", times)],
    components=[
        rr.Points3D.indicator(),
        rr.components.Position3DBatch(positions).partition([10] * 10),
        rr.components.ColorBatch(colors).partition([10] * 10),
        rr.components.RadiusBatch(radii).partition([10] * 10),
    ],
)

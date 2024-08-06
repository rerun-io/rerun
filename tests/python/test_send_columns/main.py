#!/usr/bin/env python3
"""Log different temporal batches of data via the `send_columns` API."""

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


rng = np.random.default_rng(12345)

times = np.arange(0, 10)
positions = rng.uniform(-5, 5, size=[100, 3])
colors = rng.uniform(0, 255, size=[100, 3])
radii = rng.uniform(0, 1, size=[100])

rr.send_columns(
    "points",
    times=[rr.TimeSequenceBatch("step", times)],
    components=[
        rr.Points3D.indicator(),
        rr.components.Position3DBatch(positions).partition([10] * 10),
        rr.components.ColorBatch(colors).partition([10] * 10),
        rr.components.RadiusBatch(radii).partition([10] * 10),
    ],
)

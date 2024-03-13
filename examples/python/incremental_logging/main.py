#!/usr/bin/env python3
"""Showcases how to incrementally log data belonging to the same archetype, and re-use some or all of it across frames."""
from __future__ import annotations

import argparse

import numpy as np
import rerun as rr
from numpy.random import default_rng

parser = argparse.ArgumentParser(description="Showcases how to incrementally log data belonging to the same archetype.")
rr.script_add_args(parser)
args = parser.parse_args()

rr.script_setup(args, "rerun_example_incremental_logging")

# TODO(#5264): just log one once clamp-to-edge semantics land.
colors = rr.components.ColorBatch(np.repeat(0xFF0000FF, 10))
radii = rr.components.RadiusBatch(np.repeat(0.1, 10))

# Only log colors and radii once.
rr.set_time_sequence("frame_nr", 0)
rr.log_components("points", [colors, radii])
# Logging timelessly would also work.
# rr.log_components("points", [colors, radii], timeless=True)

rng = default_rng(12345)

# Then log only the points themselves each frame.
#
# They will automatically re-use the colors and radii logged at the beginning.
for i in range(10):
    rr.set_time_sequence("frame_nr", i)
    rr.log("points", rr.Points3D(rng.uniform(-5, 5, size=[10, 3])))

rr.script_teardown(args)

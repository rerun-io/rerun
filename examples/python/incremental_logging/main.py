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


README = """\
# Incremental Logging

This example showcases how to incrementally log data belonging to the same archetype, and re-use some or all of it across frames.

It was logged with the following code:
```python
colors = rr.components.ColorBatch(np.repeat(0xFF0000FF, 10))
radii = rr.components.RadiusBatch(np.repeat(0.1, 10))

# Only log colors and radii once.
rr.set_time_sequence("frame_nr", 0)
rr.log_components("points", [colors, radii])

rng = default_rng(12345)

# Then log only the points themselves each frame.
#
# They will automatically re-use the colors and radii logged at the beginning.
for i in range(10):
    rr.set_time_sequence("frame_nr", i)
    rr.log("points", rr.Points3D(rng.uniform(-5, 5, size=[10, 3])))
```

Move the time cursor around, and notice how the colors and radii from frame 0 are still picked up by later frames, while the points themselves keep changing every frame.
"""

# ---

rr.script_setup(args, "rerun_example_incremental_logging")

rr.log("readme", rr.TextDocument(README, media_type=rr.MediaType.MARKDOWN), static=True)

# TODO(#5264): just log one once clamp-to-edge semantics land.
colors = rr.components.ColorBatch(np.repeat(0xFF0000FF, 10))
radii = rr.components.RadiusBatch(np.repeat(0.1, 10))

# Only log colors and radii once.
rr.set_time_sequence("frame_nr", 0)
rr.log_components("points", [colors, radii])
# Logging statically would also work.
# rr.log_components("points", [colors, radii], static=True)

rng = default_rng(12345)

# Then log only the points themselves each frame.
#
# They will automatically re-use the colors and radii logged at the beginning.
for i in range(10):
    rr.set_time_sequence("frame_nr", i)
    rr.log("points", rr.Points3D(rng.uniform(-5, 5, size=[10, 3])))

rr.script_teardown(args)

"""
Stress test for cross-recording garbage collection.

Logs many medium-sized recordings that contain a lot of small-ish rows.

Usage:
- Start a Rerun Viewer in release mode with 500MiB of memory limit:
  `cargo r -p rerun-cli --release --no-default-features --features native_viewer -- --memory-limit 500MiB`
- Open the memory panel to see what's going on.
- Run this script.
- You should see recordings coming in and going out in a ringbuffer-like rolling fashion.
"""

from __future__ import annotations

import rerun as rr
from numpy.random import default_rng

rng = default_rng(12345)

for i in range(20000000):
    rr.init("rerun_example_recording_gc", recording_id=f"image-rec-{i}", spawn=True)
    for j in range(10000):
        rr.set_time("frame", sequence=j)
        positions = rng.uniform(-5, 5, size=[1000, 3])
        colors = rng.uniform(0, 255, size=[1000, 3])
        radii = rng.uniform(0, 1, size=[1000])
        rr.log("points", rr.Points3D(positions, colors=colors, radii=radii))

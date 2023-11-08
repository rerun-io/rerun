"""
Stress test for cross-recording garbage collection.

Logs many large recordings that contain a lot of large rows.

Usage:
- Start a Rerun Viewer in release mode with 2GiB of memory limit:
  `cargo r -p rerun-cli --release --no-default-features --features native_viewer -- --memory-limit 2GiB`
- Open the memory panel to see what's going on.
- Run this script.
- You should see recordings coming in and going out in a ringbuffer-like rolling fashion.
"""
from __future__ import annotations

import rerun as rr
from numpy.random import default_rng

rng = default_rng(12345)

for i in range(0, 20000000):
    rr.init("rerun_example_recording_gc", recording_id=f"image-rec-{i}", spawn=True)
    for j in range(0, 10000):
        positions = rng.uniform(-5, 5, size=[10000000, 3])
        colors = rng.uniform(0, 255, size=[10000000, 3])
        radii = rng.uniform(0, 1, size=[10000000])
        rr.log("points", rr.Points3D(positions, colors=colors, radii=radii))

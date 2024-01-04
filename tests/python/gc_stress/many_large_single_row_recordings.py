"""
Stress test for cross-recording garbage collection.

Logs many large recordings that contain a single large row.

Usage:
- Start a Rerun Viewer in release mode with 2GiB of memory limit:
  `cargo r -p rerun-cli --release --no-default-features --features native_viewer -- --memory-limit 2GiB`
- Open the memory panel to see what's going on.
- Run this script.
- You should see recordings coming in and going out in a ringbuffer-like rolling fashion.
"""
from __future__ import annotations

import time

import rerun as rr
from numpy.random import default_rng

rng = default_rng(12345)

for i in range(0, 20000000):
    rr.init("rerun_example_recording_gc", recording_id=f"recording-gc-rec-{i}", spawn=True)

    positions = rng.uniform(-5, 5, size=[10000000, 3])
    colors = rng.uniform(0, 255, size=[10000000, 3])
    radii = rng.uniform(0, 1, size=[10000000])
    rr.log("points", rr.Points3D(positions, colors=colors, radii=radii))

    # Sleep because large single row recordings will absolutely destroy the viewer.
    # TODO(#4185): Investigate this.
    time.sleep(1)

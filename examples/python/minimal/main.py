#!/usr/bin/env python3
"""Demonstrates the most barebone usage of the Rerun SDK."""
from __future__ import annotations

import sys

import numpy as np
import rerun as rr  # pip install rerun-sdk

# sanity-check since all other example scripts take arguments:
assert len(sys.argv) == 1, f"{sys.argv[0]} does not take any arguments"

rr.init("rerun_example_minimal", spawn=True)

positions = np.vstack([xyz.ravel() for xyz in np.mgrid[3 * [slice(-10, 10, 10j)]]]).T
colors = np.vstack([rgb.ravel() for rgb in np.mgrid[3 * [slice(0, 255, 10j)]]]).astype(np.uint8).T

rr.log("my_points", rr.Points3D(positions, colors=colors, radii=0.5))

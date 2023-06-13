#!/usr/bin/env python3
"""Demonstrates the most barebone usage of the Rerun SDK, with standard options."""
from __future__ import annotations

import argparse

import numpy as np
import rerun as rr  # pip install rerun-sdk

parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
rr.script_add_args(parser)
args = parser.parse_args()

rr.script_setup(args, "minimal_options")

positions = np.vstack([xyz.ravel() for xyz in np.mgrid[3 * [slice(-10, 10, 10j)]]]).T
colors = np.vstack([rgb.ravel() for rgb in np.mgrid[3 * [slice(0, 255, 10j)]]]).astype(np.uint8).T

rr.log_points("my_points", positions=positions, colors=colors, radii=0.5)

rr.script_teardown(args)

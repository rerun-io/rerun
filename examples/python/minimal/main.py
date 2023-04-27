#!/usr/bin/env python3

"""Demonstrates the most barebone usage of the Rerun SDK."""


import numpy as np
import rerun as rr

_, unknown = __import__("argparse").ArgumentParser().parse_known_args()
[__import__("logging").warning(f"unknown arg: {arg}") for arg in unknown]

rr.init("minimal", spawn=True)

positions = np.vstack([xyz.ravel() for xyz in np.mgrid[3 * [slice(-5, 5, 10j)]]]).T
colors = np.vstack([rgb.ravel() for rgb in np.mgrid[3 * [slice(0, 255, 10j)]]]).astype(np.uint8).T

rr.log_points("my_points", positions=positions, colors=colors)

#!/usr/bin/env python3

"""Demonstrates the most barebone usage of the Rerun SDK, with standard options."""


import argparse

import numpy as np
import rerun as rr

parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
rr.script_add_args(parser)
args, unknown = parser.parse_known_args()
[__import__("logging").warning(f"unknown arg: {arg}") for arg in unknown]

rr.script_setup(args, "minimal_options")

positions = np.vstack([xyz.ravel() for xyz in np.mgrid[3 * [slice(-5, 5, 10j)]]]).T
colors = np.vstack([rgb.ravel() for rgb in np.mgrid[3 * [slice(0, 255, 10j)]]]).astype(np.uint8).T

rr.log_points("my_points", positions=positions, colors=colors)

rr.script_teardown(args)

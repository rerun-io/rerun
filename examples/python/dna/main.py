#!/usr/bin/env python3
"""
The example from our Getting Started page.

`examples/python/dna/main.py`
"""
from __future__ import annotations

import argparse
from math import tau

import numpy as np
import rerun as rr  # pip install rerun-sdk
from rerun_demo.data import build_color_spiral
from rerun_demo.util import bounce_lerp, interleave


def log_data() -> None:
    rr.set_time_seconds("stable_time", 0)

    NUM_POINTS = 100

    # points and colors are both np.array((NUM_POINTS, 3))
    points1, colors1 = build_color_spiral(NUM_POINTS)
    points2, colors2 = build_color_spiral(NUM_POINTS, angular_offset=tau * 0.5)
    rr.log("helix/structure/left", rr.Points3D(points1, colors=colors1, radii=0.08))
    rr.log("helix/structure/right", rr.Points3D(points2, colors=colors2, radii=0.08))

    points = interleave(points1, points2)
    rr.log("helix/structure/scaffolding", rr.LineStrips3D(points.reshape(-1, 3), colors=[128, 128, 128]))

    time_offsets = np.random.rand(NUM_POINTS)
    for i in range(400):
        time = i * 0.01
        rr.set_time_seconds("stable_time", time)

        times = np.repeat(time, NUM_POINTS) + time_offsets
        beads = [bounce_lerp(points1[n], points2[n], times[n]) for n in range(NUM_POINTS)]
        colors = [[int(bounce_lerp(80, 230, times[n] * 2))] for n in range(NUM_POINTS)]
        rr.log(
            "helix/structure/scaffolding/beads", rr.Points3D(beads, radii=0.06, colors=np.repeat(colors, 3, axis=-1))
        )

        rr.log(
            "helix/structure",
            rr.TranslationRotationScale3D(rotation=rr.RotationAxisAngle(axis=[0, 0, 1], radians=time / 4.0 * tau)),
        )


def main() -> None:
    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_dna_abacus")
    log_data()
    rr.script_teardown(args)


if __name__ == "__main__":
    main()

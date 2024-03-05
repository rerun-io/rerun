#!/usr/bin/env python3

"""Logs a `LineStrips2D` archetype for roundtrip checks."""

from __future__ import annotations

import argparse

import numpy as np
import rerun as rr


def main() -> None:
    points = np.array([[0, 0], [2, 1], [4, -1], [6, 0]]).reshape([2, 2, 2])
    radii = np.array([0.42, 0.43], dtype=np.float32)
    colors = np.array(
        [
            0xAA0000CC,
            0x00BB00DD,
        ],
        dtype=np.uint32,
    )
    labels = ["hello", "friend"]
    draw_order = 300
    class_ids = np.array([126, 127], dtype=np.uint64)

    line_strips2d = rr.LineStrips2D(
        points,
        radii=radii,
        colors=colors,
        labels=labels,
        draw_order=draw_order,
        class_ids=class_ids,
    )

    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_roundtrip_line_strips2d")

    rr.log("line_strips2d", line_strips2d)
    # Hack to establish 2d view bounds
    rr.log("rect", rr.Boxes2D(centers=[0, 0], half_sizes=[10, 10]))

    rr.script_teardown(args)


if __name__ == "__main__":
    main()

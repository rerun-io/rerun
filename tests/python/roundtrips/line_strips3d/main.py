#!/usr/bin/env python3

"""Logs a `LineStrips3D` archetype for roundtrip checks."""

from __future__ import annotations

import argparse

import numpy as np
import rerun as rr


def main() -> None:
    points = np.array([[0.0, 0.0, 0.0], [2.0, 1.0, -1.0], [4.0, -1.0, 3.0], [6.0, 0.0, 1.5]]).reshape([2, 2, 3])
    radii = np.array([0.42, 0.43], dtype=np.float32)
    colors = np.array(
        [
            0xAA0000CC,
            0x00BB00DD,
        ],
        dtype=np.uint32,
    )
    labels = ["hello", "friend"]
    class_ids = np.array([126, 127], dtype=np.uint64)

    line_strips3d = rr.LineStrips3D(
        points,
        radii=radii,
        colors=colors,
        labels=labels,
        class_ids=class_ids,
    )

    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_roundtrip_line_strips3d")

    rr.log("line_strips3d", line_strips3d)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()

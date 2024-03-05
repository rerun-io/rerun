#!/usr/bin/env python3

"""Logs a `Points3D` archetype for roundtrip checks."""

from __future__ import annotations

import argparse

import numpy as np
import rerun as rr


def main() -> None:
    points = np.array([1, 2, 3, 4, 5, 6], dtype=np.float32)
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
    keypoint_ids = np.array([2, 3], dtype=np.uint64)

    points3d = rr.Points3D(
        points,
        radii=radii,
        colors=colors,
        labels=labels,
        class_ids=class_ids,
        keypoint_ids=keypoint_ids,
    )

    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_roundtrip_points3d")

    rr.log("points3d", points3d)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()

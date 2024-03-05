#!/usr/bin/env python3

"""Logs a `Box3D` archetype for roundtrip checks."""

from __future__ import annotations

import argparse

import numpy as np
import rerun as rr
from rerun.datatypes import Angle, Quaternion, RotationAxisAngle


def main() -> None:
    half_sizes = np.array([[10, 9, 8], [5, -5, 5]])
    centers = np.array([[0, 0, 0], [-1, 1, -2]])
    rotations = [Quaternion(xyzw=[0, 1, 2, 3]), RotationAxisAngle([0, 1, 2], Angle(deg=45))]
    colors = np.array(
        [
            0xAA0000CC,
            0x00BB00DD,
        ],
        dtype=np.uint32,
    )
    radii = np.array([0.1, 0.01], dtype=np.float32)
    labels = ["hello", "friend"]
    class_ids = np.array([126, 127], dtype=np.uint64)

    boxes3d = rr.Boxes3D(
        half_sizes=half_sizes,
        centers=centers,
        rotations=rotations,
        colors=colors,
        labels=labels,
        radii=radii,
        class_ids=class_ids,
    )

    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_roundtrip_box3d")

    rr.log("boxes3d", boxes3d)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()

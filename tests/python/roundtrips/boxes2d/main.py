#!/usr/bin/env python3

"""Logs a `Box2D` archetype for roundtrip checks."""

from __future__ import annotations

import argparse

import numpy as np
import rerun as rr


def main() -> None:
    half_sizes = np.array([[10, 9], [5, -5]])
    centers = np.array([[0, 0], [-1, 1]])
    colors = np.array(
        [
            0xAA0000CC,
            0x00BB00DD,
        ],
        dtype=np.uint32,
    )
    radii = np.array([0.1, 1.0], dtype=np.float32)
    labels = ["hello", "friend"]
    draw_order = 300
    class_ids = np.array([126, 127], dtype=np.uint64)
    instance_keys = np.array([66, 666], dtype=np.uint64)

    boxes2d = rr.Boxes2D(
        half_sizes=half_sizes,
        centers=centers,
        colors=colors,
        labels=labels,
        radii=radii,
        draw_order=draw_order,
        class_ids=class_ids,
        instance_keys=instance_keys,
    )

    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_roundtrip_box2d")

    rr.log("boxes2d", boxes2d)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()

#!/usr/bin/env python3

"""Logs a `Arrows2D` archetype for roundtrip checks."""

from __future__ import annotations

import argparse

import numpy as np
import rerun as rr


def main() -> None:
    origins = [[1, 2], [10, 20]]
    vectors = [[4, 5], [40, 50]]
    radii = np.array([0.1, 1.0], dtype=np.float32)
    colors = np.array(
        [
            0xAA0000CC,
            0x00BB00DD,
        ],
        dtype=np.uint32,
    )
    labels = ["hello", "friend"]
    class_ids = np.array([126, 127], dtype=np.uint64)

    arrows2d = rr.Arrows2D(
        vectors=vectors,
        origins=origins,
        radii=radii,
        colors=colors,
        labels=labels,
        class_ids=class_ids,
    )

    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_roundtrip_arrows2d")

    rr.log("arrows2d", arrows2d)

    rr.script_teardown(args)


if __name__ == "__main__":
    main()

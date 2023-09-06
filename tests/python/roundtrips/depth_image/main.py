#!/usr/bin/env python3

"""Logs a `DepthImage` archetype for roundtrip checks."""

from __future__ import annotations

import argparse

import numpy as np
import rerun as rr
import rerun.experimental as rr2


def main() -> None:
    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_roundtrip_depth_image")

    # 3x2 image. Each pixel is i*j
    image = np.zeros((2, 3), dtype=np.uint8)
    for i in range(3):
        for j in range(2):
            image[j, i] = i * j

    rr2.log("depth_image", rr2.DepthImage(data=image, meter=1000))

    rr.script_teardown(args)


if __name__ == "__main__":
    main()

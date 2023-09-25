#!/usr/bin/env python3

"""Logs a `DepthImage` archetype for roundtrip checks."""

from __future__ import annotations

import argparse

import numpy as np
import rerun as rr


def main() -> None:
    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_roundtrip_depth_image")

    # 3x2 image. Each pixel is i*j
    image = np.zeros((2, 3), dtype=np.uint8)
    image[0, :] = [0, 1, 2]
    image[1, :] = [3, 4, 5]

    rr.log("depth_image", rr.DepthImage(data=image, meter=1000))

    rr.script_teardown(args)


if __name__ == "__main__":
    main()

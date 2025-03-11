#!/usr/bin/env python3

"""Logs a `Image` archetype for roundtrip checks."""

from __future__ import annotations

import argparse

import numpy as np
import rerun as rr


def main() -> None:
    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_roundtrip_image")

    # h=2 w=3 c=3 image. Red channel = x. Green channel = y. Blue channel = 128.
    image = np.zeros((2, 3, 3), dtype=np.uint8)
    for i in range(3):
        image[:, i, 0] = i
    for i in range(2):
        image[i, :, 1] = i
    image[:, :, 2] = 128

    rr.log("image", rr.Image(image))

    # h=4, w=5 mono image. Pixel = x * y * 123.4
    image2 = np.zeros((4, 5), dtype=np.float16)
    for i in range(4):
        for j in range(5):
            image2[i, j] = i * j * 123.4

    rr.log("image_f16", rr.Image(image2))

    rr.script_teardown(args)


if __name__ == "__main__":
    main()

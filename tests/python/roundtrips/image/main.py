#!/usr/bin/env python3

"""Logs a `Image` archetype for roundtrip checks."""

from __future__ import annotations

import argparse

import numpy as np
import rerun as rr
from rerun.datatypes import TensorData


def main() -> None:
    parser = argparse.ArgumentParser(description="Logs rich data using the Rerun SDK.")
    rr.script_add_args(parser)
    args = parser.parse_args()

    rr.script_setup(args, "rerun_example_roundtrip_image")

    # 2x3x3 image. Red channel = x. Green channel = y. Blue channel = 128.
    image = np.zeros((2, 3, 3), dtype=np.uint8)
    for i in range(3):
        image[:, i, 0] = i
    for i in range(2):
        image[i, :, 1] = i
    image[:, :, 2] = 128

    image = TensorData(array=image)

    rr.log("image", rr.Image(image))

    # 4x5 mono image. Pixel = x * y * 123.4
    image = np.zeros((4, 5), dtype=np.float16)
    for i in range(4):
        for j in range(5):
            image[i, j] = i * j * 123.4

    image = TensorData(array=image)

    rr.log("image_f16", rr.Image(image))

    rr.script_teardown(args)


if __name__ == "__main__":
    main()
